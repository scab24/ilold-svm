use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;
use reedline::{
    Completer, DefaultHinter, FileBackedHistory, Prompt, PromptEditMode,
    PromptHistorySearch, PromptHistorySearchStatus, Reedline, Signal, Span, Suggestion,
};

use ilold_core::classify::entry_points::AccessLevel;
use ilold_core::exploration::commands::CommandResult;

use crate::colors::*;

pub async fn run(paths: Vec<PathBuf>, port: u16, max_seq_depth: usize) -> Result<()> {
    println!("Analyzing {} file(s)...", paths.len());
    let (state, actual_port) = ilold_web::start_server(paths, port, max_seq_depth).await?;

    let contract_name = state.project.contracts.iter()
        .find(|c| c.kind != ilold_core::model::contract::ContractKind::Interface)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "unknown".into());

    let function_names: Vec<String> = state.project.contracts.iter()
        .find(|c| c.name == contract_name)
        .map(|c| c.functions.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();

    let func_count = function_names.len();

    println!(
        "Ready: {} functions analyzed\n",
        func_count,
    );
    println!("  ┌─────────────────────────────────────────┐");
    println!("  │  {} — {}",
        c_bright("ilold explore"),
        c_accent(&contract_name),
    );
    println!("  │  {} functions | Type {} for help",
        func_count,
        c_accent("?"),
    );
    println!("  │  Web UI: {}",
        c_muted(&format!("http://localhost:{actual_port}")),
    );
    println!("  └─────────────────────────────────────────┘\n");

    let handle = tokio::runtime::Handle::current();
    let repl_thread = std::thread::spawn(move || {
        repl_loop(handle, actual_port, &contract_name, &function_names);
    });

    repl_thread.join().map_err(|_| anyhow::anyhow!("REPL thread panicked"))?;
    Ok(())
}

fn repl_loop(handle: tokio::runtime::Handle, port: u16, contract: &str, functions: &[String]) {
    let history_path = dirs::home_dir()
        .map(|h| h.join(".ilold").join("history"))
        .unwrap_or_else(|| PathBuf::from(".ilold_history"));

    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let history = Box::new(
        FileBackedHistory::with_file(500, history_path).expect("Failed to create history"),
    );

    let completer = Box::new(IloldCompleter {
        functions: functions.to_vec(),
    });

    let mut editor = Reedline::create()
        .with_history(history)
        .with_completer(completer)
        .with_hinter(Box::new(DefaultHinter::default().with_style(
            nu_ansi_term::Style::new().fg(nu_ansi_term::Color::DarkGray),
        )));

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut steps: Vec<String> = Vec::new();
    let mut scenario_name: Option<String> = None;

    let mut prompt = IloldPrompt {
        contract: contract.to_string(),
        steps: Vec::new(),
    };

    loop {
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim();
                if line.is_empty() { continue; }

                match handle_input(
                    line, &handle, &client, &base_url, contract,
                    &mut steps, &mut scenario_name,
                ) {
                    InputResult::Continue => {}
                    InputResult::Quit => break,
                    InputResult::UpdatePrompt => {
                        prompt.steps = steps.clone();
                    }
                }
            }
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => break,
            Err(e) => {
                eprintln!("Error: {e}");
                break;
            }
        }
    }
}

enum InputResult {
    Continue,
    Quit,
    UpdatePrompt,
}

fn handle_input(
    line: &str,
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
    steps: &mut Vec<String>,
    scenario_name: &mut Option<String>,
) -> InputResult {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        "?" | "h" | "help" => { print_help(); InputResult::Continue }
        "q" | "quit" | "exit" => InputResult::Quit,
        "browser" => {
            println!("  {}", c_muted("Web UI not yet available in explore mode."));
            println!("  API running at {base_url}/api/");
            InputResult::Continue
        }
        "sc" | "scenario" => {
            if arg.is_empty() {
                match scenario_name {
                    Some(name) => println!("  Current scenario: {}", c_accent(name)),
                    None => println!("  No scenario set. Usage: scenario <name>"),
                }
            } else {
                *scenario_name = Some(arg.to_string());
                println!("  Scenario: {}", c_accent(arg));
            }
            InputResult::Continue
        }

        "c" | "call" => {
            if arg.is_empty() {
                println!("  Usage: call <function>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({
                "contract": contract,
                "command": { "Call": { "func": arg } }
            });
            match send_command(handle, client, base_url, &body) {
                Ok(result) => {
                    if let CommandResult::StepAdded { function, .. } = &result {
                        steps.push(function.clone());
                    }
                    print_result(&result, steps);
                    if matches!(&result, CommandResult::StepAdded { .. }) {
                        return InputResult::UpdatePrompt;
                    }
                }
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "b" | "back" => {
            let body = serde_json::json!({
                "contract": contract,
                "command": "Back"
            });
            match send_command(handle, client, base_url, &body) {
                Ok(result) => {
                    if matches!(&result, CommandResult::StepRemoved { .. }) {
                        steps.pop();
                    }
                    print_result(&result, steps);
                    if matches!(&result, CommandResult::StepRemoved { .. }) {
                        return InputResult::UpdatePrompt;
                    }
                }
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "cl" | "clear" => {
            if !steps.is_empty() {
                println!("  Clear {} steps? (y/n)", steps.len());
                let mut input = String::new();
                match std::io::stdin().read_line(&mut input) {
                    Ok(_) if input.trim().eq_ignore_ascii_case("y") => { /* proceed */ }
                    _ => {
                        println!("  Cancelled.");
                        return InputResult::Continue;
                    }
                }
            }
            let body = serde_json::json!({
                "contract": contract,
                "command": "Clear"
            });
            match send_command(handle, client, base_url, &body) {
                Ok(result) => {
                    print_result(&result, steps);
                    steps.clear();
                    return InputResult::UpdatePrompt;
                }
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "s" | "state" => {
            send_and_print(handle, client, base_url, contract, "State", steps);
            InputResult::Continue
        }
        "f" | "functions" => {
            send_and_print(handle, client, base_url, contract, "Functions", steps);
            InputResult::Continue
        }
        "ss" | "session" => {
            send_and_print(handle, client, base_url, contract, "Session", steps);
            InputResult::Continue
        }
        "n" | "note" => {
            if arg.is_empty() {
                println!("  Usage: note <text>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({
                "contract": contract,
                "command": { "Note": { "text": arg } }
            });
            match send_command(handle, client, base_url, &body) {
                Ok(result) => print_result(&result, steps),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "fi" | "finding" => {
            handle_finding_interactive(handle, client, base_url, contract, steps);
            InputResult::Continue
        }
        "status" => {
            let status_parts: Vec<&str> = arg.splitn(2, ' ').collect();
            if status_parts.len() < 2 {
                println!("  Usage: status <function> <reviewed|suspicious|vulnerable|clean|inprogress|notreviewed>");
                return InputResult::Continue;
            }
            let normalized = match normalize_status(status_parts[1]) {
                Some(s) => s,
                None => {
                    println!("  {}", c_danger("Invalid status. Valid: reviewed, suspicious, vulnerable, clean, inprogress, notreviewed"));
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({
                "contract": contract,
                "command": { "Status": { "func": status_parts[0], "status": normalized } }
            });
            match send_command(handle, client, base_url, &body) {
                Ok(result) => print_result(&result, steps),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "w" | "who" => {
            if arg.is_empty() {
                println!("  Usage: who <variable>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({
                "contract": contract,
                "command": { "Who": { "variable": arg } }
            });
            match send_command(handle, client, base_url, &body) {
                Ok(result) => print_result(&result, steps),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }

        "v" | "vars" => {
            match send_get(handle, client, &format!("{base_url}/api/contract/{contract}")) {
                Ok(val) => print_vars(&val),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }

        "i" | "info" => {
            if arg.is_empty() {
                println!("  Usage: info <function>");
                return InputResult::Continue;
            }
            match send_get(handle, client, &format!("{base_url}/api/session/function/{contract}/{arg}")) {
                Ok(val) => print_narrative(&val),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "seq" | "sequence" => {
            match send_get(handle, client, &format!("{base_url}/api/session/sequence")) {
                Ok(val) => print_sequence_narrative(&val),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "st" | "step" => {
            if arg.is_empty() {
                println!("  Usage: step <index>");
                return InputResult::Continue;
            }
            match send_get(handle, client, &format!("{base_url}/api/session/step/{arg}/narrative")) {
                Ok(val) => print_narrative(&val),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }

        "fl" | "findings" => {
            print_findings_list(handle, client, base_url, contract);
            InputResult::Continue
        }
        "ex" | "export" => {
            println!("  {}", c_muted("Export not yet implemented (session persistence needed)"));
            InputResult::Continue
        }

        _ => {
            println!("  Unknown command: {}. Type {} for help.", c_danger(cmd.as_str()), c_accent("?"));
            InputResult::Continue
        }
    }
}

fn send_command(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    body: &serde_json::Value,
) -> Result<CommandResult, String> {
    handle.block_on(async {
        client.post(format!("{base_url}/api/cmd"))
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?
            .json::<CommandResult>()
            .await
            .map_err(|e| format!("Parse failed: {e}"))
    })
}

fn send_get(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    url: &str,
) -> Result<serde_json::Value, String> {
    handle.block_on(async {
        let resp = client.get(url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(if body.is_empty() {
                format!("Server error: {status}")
            } else {
                body
            });
        }

        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Parse failed: {e}"))
    })
}

fn send_and_print(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
    command: &str,
    steps: &[String],
) {
    let body = serde_json::json!({ "contract": contract, "command": command });
    match send_command(handle, client, base_url, &body) {
        Ok(result) => print_result(&result, steps),
        Err(e) => eprintln!("  {}", c_danger(&e)),
    }
}

fn normalize_severity(input: &str) -> Option<&'static str> {
    match input.to_lowercase().as_str() {
        "critical" => Some("Critical"),
        "high" => Some("High"),
        "medium" => Some("Medium"),
        "low" => Some("Low"),
        "informational" | "info" => Some("Informational"),
        _ => None,
    }
}

fn normalize_status(input: &str) -> Option<&'static str> {
    match input.to_lowercase().as_str() {
        "reviewed" => Some("Reviewed"),
        "suspicious" => Some("Suspicious"),
        "vulnerable" => Some("Vulnerable"),
        "clean" => Some("Clean"),
        "inprogress" => Some("InProgress"),
        "notreviewed" => Some("NotReviewed"),
        _ => None,
    }
}

fn read_prompt(label: &str) -> Option<String> {
    println!("  {} {}", label, c_muted("(empty to cancel)"));
    print!("  > ");
    std::io::Write::flush(&mut std::io::stdout()).ok();
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(0) | Err(_) => None,
        Ok(_) => {
            let trimmed = input.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        }
    }
}

fn handle_finding_interactive(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
    steps: &[String],
) {
    let severity_input = match read_prompt("Severity (critical/high/medium/low/info):") {
        Some(s) => s,
        None => { println!("  {}", c_muted("Cancelled.")); return; }
    };
    let severity = match normalize_severity(&severity_input) {
        Some(s) => s.to_string(),
        None => {
            println!("  {}", c_danger("Invalid severity. Valid: critical, high, medium, low, info"));
            return;
        }
    };

    let title = match read_prompt("Title:") {
        Some(t) => t,
        None => { println!("  {}", c_muted("Cancelled.")); return; }
    };

    let description = read_prompt("Description (optional):").unwrap_or_default();

    let body = serde_json::json!({
        "contract": contract,
        "command": {
            "Finding": {
                "severity": severity,
                "title": title,
                "description": description
            }
        }
    });
    match send_command(handle, client, base_url, &body) {
        Ok(result) => print_result(&result, steps),
        Err(e) => eprintln!("  {}", c_danger(&e)),
    }
}

fn print_findings_list(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
) {
    let body = serde_json::json!({ "contract": contract, "command": "Session" });
    match send_command(handle, client, base_url, &body) {
        Ok(CommandResult::SessionView { findings_count, .. }) => {
            if findings_count == 0 {
                println!("  No findings recorded yet.");
            } else {
                println!("  {} finding(s) recorded. Use {} to export.", findings_count, c_accent("export"));
            }
        }
        _ => println!("  Could not retrieve findings."),
    }
}

// ─── Output formatting ─────────────────────────────────────────────────────

fn print_result(result: &CommandResult, steps: &[String]) {
    match result {
        CommandResult::StepAdded { step_index, function, access, state_changed } => {
            let badge = access_colored(access);
            println!();
            println!("  {}[ STEP {}: {} ]{}", "═".truecolor(60, 70, 90), step_index, c_bright(function), "═".truecolor(60, 70, 90));
            println!("  {} {} {}", badge, c_bright(function), format_access_detail(access));
            if !state_changed.is_empty() {
                println!("  {}[ STATE ]{}", "═".truecolor(60, 70, 90), "═".truecolor(60, 70, 90));
                for var in state_changed {
                    println!("    {} {}", c_danger("✏"), c_warn(var));
                }
            }
            println!("  {}[ SEQUENCE ]{}", "═".truecolor(60, 70, 90), "═".truecolor(60, 70, 90));
            for (i, name) in steps.iter().enumerate() {
                if i == *step_index {
                    println!("  {} {}. {}  ← current", ">".truecolor(100, 160, 110), i, c_bright(name));
                } else {
                    println!("    {}. {}", i, c_muted(name));
                }
            }
            println!();
        }
        CommandResult::StepRemoved { remaining } => {
            println!("  Step removed. {} remaining.", remaining);
            if !steps.is_empty() {
                println!("  {}[ SEQUENCE ]{}", "═".truecolor(60, 70, 90), "═".truecolor(60, 70, 90));
                for (i, name) in steps.iter().enumerate() {
                    let marker = if i == steps.len() - 1 { " ← current" } else { "" };
                    println!("    {}. {}{}", i, c_muted(name), c_ok(marker));
                }
            }
            println!();
        }
        CommandResult::Cleared => {
            println!("  {}", c_ok("Session cleared."));
        }
        CommandResult::StateView { summary } => {
            if summary.is_empty() {
                println!("  No state changes yet. Use {} to add steps.", c_accent("call <func>"));
                return;
            }
            println!();
            println!("  {}[ STATE ]{}", "═".truecolor(60, 70, 90), "═".truecolor(60, 70, 90));
            for var in summary {
                println!("  {} {}", c_bright(&var.variable), "");
                for change in &var.changes {
                    println!("    {}", c_muted(change));
                }
            }
            println!();
        }
        CommandResult::FunctionList { functions } => {
            println!();
            for (name, access) in functions {
                if name.is_empty() { continue; }
                let badge = access_colored(access);
                println!("  {badge} {}", c_accent(name));
            }
            println!();
        }
        CommandResult::FindingAdded { id } => {
            println!("  {} Finding {} added", c_ok("✓"), c_accent(id));
        }
        CommandResult::NoteAdded => {
            println!("  {} Note added", c_ok("✓"));
        }
        CommandResult::StatusUpdated => {
            println!("  {} Status updated", c_ok("✓"));
        }
        CommandResult::SessionView { contract, steps: session_steps, findings_count } => {
            println!();
            println!("  Contract: {}", c_bright(contract));
            println!("  Steps:    {}", if session_steps.is_empty() {
                c_muted("(empty)").to_string()
            } else {
                session_steps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ")
            });
            println!("  Findings: {}", findings_count);
            println!();
        }
        CommandResult::VariableInfo { variable, writers, readers } => {
            println!();
            println!("  {} {}", c_bright("who:"), c_accent(variable));
            if !writers.is_empty() {
                println!("    {}:", c_warn("Writers"));
                for (name, access) in writers {
                    println!("      {} {}", access_colored(access), c_accent(name));
                }
            }
            if !readers.is_empty() {
                println!("    {}:", c_muted("Readers"));
                for (name, access) in readers {
                    println!("      {} {}", access_colored(access), c_muted(name));
                }
            }
            println!();
        }
        CommandResult::Error { message } => {
            println!("  {}", c_danger(message));
        }
    }
}

fn format_access_detail(access: &AccessLevel) -> String {
    match access {
        AccessLevel::Public => "external".truecolor(110, 120, 140).to_string(),
        AccessLevel::Restricted { role } => format!("{}", c_warn(&format!("restricted({role})"))),
        AccessLevel::Internal => "internal".truecolor(110, 120, 140).to_string(),
        AccessLevel::Special { kind } => format!("{}", c_muted(&format!("special({kind})"))),
    }
}

fn print_vars(val: &serde_json::Value) {
    let vars = match val.get("state_vars").and_then(|v| v.as_array()) {
        Some(v) => v,
        None => { println!("  No state variables found."); return; }
    };
    println!();
    for v in vars {
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let type_name = v.get("type_name").and_then(|n| n.as_str()).unwrap_or("?");
        let is_const = v.get("is_constant").and_then(|n| n.as_bool()).unwrap_or(false);
        let is_immut = v.get("is_immutable").and_then(|n| n.as_bool()).unwrap_or(false);

        let tag = if is_const {
            c_muted("const").to_string()
        } else if is_immut {
            c_muted("immutable").to_string()
        } else {
            c_warn("mutable").to_string()
        };

        println!("  {} {} {}", tag, c_accent(name), c_muted(type_name));
    }
    println!();
}

fn print_narrative(val: &serde_json::Value) {
    println!();
    if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
        let access = val.get("access").and_then(|v| v.as_str()).unwrap_or("");
        let mods = val.get("modifiers").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|m| m.as_str()).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let mod_str = if mods.is_empty() { String::new() } else { format!(" — {}", c_muted(&mods)) };
        println!("  {} [{}]{}", c_bright(name), c_accent(access), mod_str);
    }
    if let Some(total) = val.get("total_paths").and_then(|v| v.as_u64()) {
        let happy = val.get("happy_paths").and_then(|v| v.as_u64()).unwrap_or(0);
        let revert = val.get("revert_paths").and_then(|v| v.as_u64()).unwrap_or(0);
        println!("  ├── {} path(s): {} happy, {} revert", total, c_ok(&happy.to_string()), c_danger(&revert.to_string()));
    }
    if let Some(reads) = val.get("state_reads").and_then(|v| v.as_array()) {
        if !reads.is_empty() {
            let vars: Vec<&str> = reads.iter().filter_map(|r| r.as_str()).collect();
            println!("  ├── {} {}", c_muted("reads:"), c_muted(&vars.join(", ")));
        }
    }
    if let Some(writes) = val.get("state_writes").and_then(|v| v.as_array()) {
        if !writes.is_empty() {
            let vars: Vec<&str> = writes.iter().filter_map(|w| w.as_str()).collect();
            println!("  ├── {} {}", c_danger("writes:"), c_warn(&vars.join(", ")));
        }
    }
    if let Some(calls) = val.get("external_calls").and_then(|v| v.as_array()) {
        if !calls.is_empty() {
            let names: Vec<&str> = calls.iter().filter_map(|c| c.as_str()).collect();
            println!("  ├── {} {}", c_warn("calls:"), c_muted(&names.join(", ")));
        }
    }
    let obs = val.get("observations").and_then(|v| v.as_array());
    let has_obs = obs.map(|o| !o.is_empty()).unwrap_or(false);
    if has_obs {
        let obs = obs.unwrap();
        println!("  └── {}:", c_danger("observations"));
        for (i, o) in obs.iter().enumerate() {
            let branch = if i == obs.len() - 1 { "└── " } else { "├── " };
            if let Some(desc) = o.get("description").and_then(|v| v.as_str()) {
                println!("      {}{}", c_muted(branch), c_danger(desc));
            }
        }
    }
    println!();
}

fn print_sequence_narrative(val: &serde_json::Value) {
    println!();
    if let Some(steps) = val.get("steps").and_then(|v| v.as_array()) {
        let names: Vec<&str> = steps.iter()
            .filter_map(|s| s.get("function").and_then(|f| f.as_str()))
            .collect();
        if !names.is_empty() {
            println!("  {}", c_bright(&names.join(" → ")));
        }
    }
    if let Some(deps) = val.get("dependencies").and_then(|v| v.as_array()) {
        if !deps.is_empty() {
            println!("  {}:", c_warn("Dependencies"));
            for dep in deps {
                if let Some(desc) = dep.get("description").and_then(|v| v.as_str()) {
                    println!("    • {}", c_muted(desc));
                }
            }
        }
    }
    if let Some(obs) = val.get("observations").and_then(|v| v.as_array()) {
        if !obs.is_empty() {
            println!("  {}:", c_danger("Observations"));
            for o in obs {
                if let Some(desc) = o.get("description").and_then(|v| v.as_str()) {
                    println!("    ! {}", c_danger(desc));
                } else if let Some(desc) = o.as_str() {
                    println!("    ! {}", c_danger(desc));
                }
            }
        }
    }
    println!();
}

fn print_help() {
    println!();
    println!("  {}  Commands:", c_bright("ilold explore"));
    println!();
    println!("  {}  {}   call <func>       Add function to sequence", c_accent("c"), c_muted("|"));
    println!("  {}  {}   back              Remove last step", c_accent("b"), c_muted("|"));
    println!("  {} {}   clear             Reset sequence", c_accent("cl"), c_muted("|"));
    println!("  {}  {}   state             Show accumulated state", c_accent("s"), c_muted("|"));
    println!("  {}  {}   functions         List available functions", c_accent("f"), c_muted("|"));
    println!("  {}  {}   vars              List state variables", c_accent("v"), c_muted("|"));
    println!("  {}  {}   who <var>         Who reads/writes a variable", c_accent("w"), c_muted("|"));
    println!("  {}  {}   info <func>       Function detail (no sequence change)", c_accent("i"), c_muted("|"));
    println!("  {} {}   sequence          Sequence narrative with dependencies", c_accent("seq"), c_muted("|"));
    println!("  {} {}   step <index>      Re-inspect a specific step", c_accent("st"), c_muted("|"));
    println!("  {} {}   session           Full session state", c_accent("ss"), c_muted("|"));
    println!("  {} {}   finding           Record a finding (interactive)", c_accent("fi"), c_muted("|"));
    println!("  {}  {}   note <text>       Add note to current step", c_accent("n"), c_muted("|"));
    println!("  {} {}   scenario <name>   Name the current sequence", c_accent("sc"), c_muted("|"));
    println!("        status <f> <s>    Change review status");
    println!("  {} {}   findings          List recorded findings", c_accent("fl"), c_muted("|"));
    println!("  {} {}   export            Export findings as markdown", c_accent("ex"), c_muted("|"));
    println!("        browser           Open web UI");
    println!("  {}  {}   quit/exit          Exit", c_accent("q"), c_muted("|"));
    println!();
}

// ─── Reedline: Prompt ──────────────────────────────────────────────────────

struct IloldPrompt {
    contract: String,
    steps: Vec<String>,
}

impl Prompt for IloldPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        if self.steps.is_empty() {
            Cow::Owned(format!("ilold[{}]", self.contract))
        } else if self.steps.len() <= 3 {
            let path = self.steps.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" → ");
            Cow::Owned(format!("ilold[→ {}]", path))
        } else {
            let skipped = self.steps.len() - 2;
            Cow::Owned(format!(
                "ilold[→ {} → ...{} more → {}]",
                self.steps[0], skipped, self.steps.last().unwrap()
            ))
        }
    }

    fn render_prompt_right(&self) -> Cow<'_, str> { Cow::Borrowed("") }
    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> { Cow::Borrowed("> ") }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> { Cow::Borrowed(".. ") }
    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        match search.status {
            PromptHistorySearchStatus::Passing => Cow::Borrowed("(search) "),
            PromptHistorySearchStatus::Failing => Cow::Borrowed("(search failed) "),
        }
    }
}

// ─── Reedline: Completer ───────────────────────────────────────────────────

struct IloldCompleter {
    functions: Vec<String>,
}

impl Completer for IloldCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let line_lower = line[..pos].to_lowercase();

        let needs_func = line_lower.starts_with("c ")
            || line_lower.starts_with("call ")
            || line_lower.starts_with("i ")
            || line_lower.starts_with("info ")
            || line_lower.starts_with("w ")
            || line_lower.starts_with("who ")
            || line_lower.starts_with("status ");

        if !needs_func {
            return Vec::new();
        }

        let arg_start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let partial = &line[arg_start..pos];

        self.functions.iter()
            .filter(|f| f.starts_with(partial))
            .map(|f| Suggestion {
                value: f.clone(),
                display_override: None,
                description: None,
                style: None,
                extra: None,
                span: Span::new(arg_start, pos),
                append_whitespace: true,
                match_indices: None,
            })
            .collect()
    }
}
