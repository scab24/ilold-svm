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
use crate::fmt;

pub async fn run(paths: Vec<PathBuf>, port: u16, max_seq_depth: usize) -> Result<()> {
    println!("Analyzing {} file(s)...", paths.len());
    let (state, actual_port) = ilold_web::start_server(paths, port, max_seq_depth).await?;

    let contract_name = state.project.find_contract(None)
        .map(|c| c.name.clone())
        .unwrap_or_else(|_| "unknown".into());

    let function_names: Vec<String> = state.project.contracts.iter()
        .find(|c| c.name == contract_name)
        .map(|c| {
            state.project
                .accessible_functions(c)
                .iter()
                .map(|af| af.function.name.clone())
                .collect()
        })
        .unwrap_or_default();

    let contract_names: Vec<String> = state.project.contracts.iter()
        .map(|c| c.name.clone())
        .filter(|n| !n.is_empty())
        .collect();

    let func_count = function_names.len();

    let banner = fmt::header_box(&[
        &format!("ilold explore — {}", contract_name),
        &format!("{} functions | Type ? for help", func_count),
        &format!("Web UI: http://localhost:{}", actual_port),
    ]);
    println!("{}\n", banner);

    let handle = tokio::runtime::Handle::current();
    let state_for_thread = state.clone();
    let repl_thread = std::thread::spawn(move || {
        repl_loop(handle, actual_port, contract_name, function_names, contract_names, state_for_thread);
    });

    repl_thread.join().map_err(|_| anyhow::anyhow!("REPL thread panicked"))?;
    Ok(())
}

fn repl_loop(
    handle: tokio::runtime::Handle,
    port: u16,
    mut contract: String,
    mut functions: Vec<String>,
    contract_names: Vec<String>,
    state: std::sync::Arc<ilold_web::state::AppState>,
) {
    let history_path = dirs::home_dir()
        .map(|h| h.join(".ilold").join("history"))
        .unwrap_or_else(|| PathBuf::from(".ilold_history"));

    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let history = Box::new(
        FileBackedHistory::with_file(500, history_path).expect("Failed to create history"),
    );

    let completer = std::sync::Arc::new(std::sync::Mutex::new(IloldCompleter {
        functions: functions.clone(),
        contracts: contract_names.clone(),
    }));

    let mut editor = Reedline::create()
        .with_history(history)
        .with_completer(Box::new(CompleterWrapper(completer.clone())))
        .with_hinter(Box::new(DefaultHinter::default().with_style(
            nu_ansi_term::Style::new().fg(nu_ansi_term::Color::DarkGray),
        )));

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut steps: Vec<String> = Vec::new();
    let mut scenario_name: Option<String> = None;

    let mut prompt = IloldPrompt {
        contract: contract.clone(),
        steps: Vec::new(),
    };

    loop {
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim();
                if line.is_empty() { continue; }

                match handle_input(
                    line, &handle, &client, &base_url, &contract,
                    &mut steps, &mut scenario_name, &state,
                ) {
                    InputResult::Continue => {}
                    InputResult::Quit => break,
                    InputResult::UpdatePrompt => {
                        prompt.steps = steps.clone();
                    }
                    InputResult::SwitchContract(new_name) => {
                        contract = new_name.clone();
                        steps.clear();
                        if let Some(c) = state.project.contracts.iter().find(|c| c.name == new_name) {
                            functions = state.project
                                .accessible_functions(c)
                                .iter()
                                .map(|af| af.function.name.clone())
                                .collect();
                            if let Ok(mut comp) = completer.lock() {
                                comp.functions = functions.clone();
                            }
                        }
                        prompt.contract = contract.clone();
                        prompt.steps = Vec::new();
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
    SwitchContract(String),
}

struct CompleterWrapper(std::sync::Arc<std::sync::Mutex<IloldCompleter>>);

impl Completer for CompleterWrapper {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        self.0.lock().map(|mut c| c.complete(line, pos)).unwrap_or_default()
    }
}

fn handle_input(
    line: &str,
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
    steps: &mut Vec<String>,
    scenario_name: &mut Option<String>,
    state: &std::sync::Arc<ilold_web::state::AppState>,
) -> InputResult {
    // Allow shortcuts like `st0`, `st1`, `step2` without requiring a space.
    let normalized = split_numeric_suffix(line);
    let parts: Vec<&str> = normalized.splitn(2, ' ').collect();
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

        "ct" | "contracts" => {
            print_contracts(state, contract);
            InputResult::Continue
        }
        "use" => {
            if arg.is_empty() {
                println!("  Usage: use <contract>");
                return InputResult::Continue;
            }
            match state.project.find_contract(Some(arg)) {
                Ok(c) => {
                    let name = c.name.clone();
                    if name == contract {
                        println!("  Already using {}", c_accent(&name));
                        return InputResult::Continue;
                    }
                    println!("  {} Now using: {}", c_ok("✓"), c_accent(&name));
                    if !steps.is_empty() {
                        println!("  {}", c_muted(&format!("Cleared {} step(s) from previous contract", steps.len())));
                    }
                    InputResult::SwitchContract(name)
                }
                Err(e) => {
                    eprintln!("  {}", c_danger(&e));
                    InputResult::Continue
                }
            }
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
        "fa" | "funcs-all" => {
            send_and_print(handle, client, base_url, contract, "FunctionsAll", steps);
            InputResult::Continue
        }
        "va" | "vars-all" => {
            send_and_print(handle, client, base_url, contract, "StateVarsAll", steps);
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
            if arg.is_empty() {
                handle_finding_interactive(handle, client, base_url, contract, steps);
            } else {
                // Parse: fi <severity> <title> [description]
                let finding_parts: Vec<&str> = arg.splitn(2, ' ').collect();
                if finding_parts.len() < 2 {
                    println!("  Usage: fi <severity> <title>");
                    println!("  Or just: fi (interactive mode)");
                } else {
                    let severity_input = finding_parts[0];
                    let rest = finding_parts[1];
                    match normalize_severity(severity_input) {
                        Some(severity) => {
                            let body = serde_json::json!({
                                "contract": contract,
                                "command": {
                                    "Finding": {
                                        "severity": severity,
                                        "title": rest,
                                        "description": ""
                                    }
                                }
                            });
                            match send_command(handle, client, base_url, &body) {
                                Ok(result) => print_result(&result, steps),
                                Err(e) => eprintln!("  {}", c_danger(&e)),
                            }
                        }
                        None => {
                            println!("  {}", c_danger("Invalid severity. Valid: critical, high, medium, low, info"));
                        }
                    }
                }
            }
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
            let body = serde_json::json!({ "contract": contract, "command": "Export" });
            match send_command(handle, client, base_url, &body) {
                Ok(CommandResult::Exported { markdown }) => {
                    let filename = format!("ilold-report-{}.md", contract);
                    match std::fs::write(&filename, &markdown) {
                        Ok(_) => println!("  {} Exported to {}", c_ok("✓"), c_accent(&filename)),
                        Err(e) => eprintln!("  {} Failed to write: {}", c_danger("✗"), e),
                    }
                }
                Ok(other) => print_result(&other, steps),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }

        "save" => {
            if arg.is_empty() {
                println!("  Usage: save <name>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({ "contract": contract, "command": "SaveSession" });
            match send_command(handle, client, base_url, &body) {
                Ok(CommandResult::SessionSaved { json }) => {
                    let dir = dirs::home_dir()
                        .map(|h| h.join(".ilold").join("sessions"))
                        .unwrap_or_else(|| std::path::PathBuf::from(".ilold/sessions"));
                    std::fs::create_dir_all(&dir).ok();
                    let path = dir.join(format!("{}.json", arg));
                    match std::fs::write(&path, &json) {
                        Ok(_) => println!("  {} Saved to {}", c_ok("✓"), c_accent(&path.display().to_string())),
                        Err(e) => eprintln!("  {} Write failed: {}", c_danger("✗"), e),
                    }
                }
                Ok(other) => print_result(&other, steps),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "load" => {
            if arg.is_empty() {
                println!("  Usage: load <name>");
                return InputResult::Continue;
            }
            let dir = dirs::home_dir()
                .map(|h| h.join(".ilold").join("sessions"))
                .unwrap_or_else(|| std::path::PathBuf::from(".ilold/sessions"));
            let path = dir.join(format!("{}.json", arg));
            let json = match std::fs::read_to_string(&path) {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("  {} File not found: {} ({})", c_danger("✗"), path.display(), e);
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({ "contract": contract, "command": { "LoadSession": { "json": json } } });
            match send_command(handle, client, base_url, &body) {
                Ok(CommandResult::SessionLoaded { steps: loaded_steps, .. }) => {
                    *steps = loaded_steps;
                    println!("  {} Session loaded ({} steps)", c_ok("✓"), steps.len());
                    return InputResult::UpdatePrompt;
                }
                Ok(other) => print_result(&other, steps),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
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

fn split_numeric_suffix(line: &str) -> String {
    let prefixes = ["st", "step"];
    let first_space = line.find(' ').unwrap_or(line.len());
    let first_word = &line[..first_space];

    for p in prefixes {
        if first_word.len() > p.len()
            && first_word.to_lowercase().starts_with(p)
            && first_word[p.len()..].chars().all(|c| c.is_ascii_digit())
        {
            let (cmd, num) = first_word.split_at(p.len());
            let rest = &line[first_space..];
            return format!("{} {}{}", cmd, num, rest);
        }
    }
    line.to_string()
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

fn print_contracts(state: &std::sync::Arc<ilold_web::state::AppState>, current: &str) {
    use ilold_core::model::contract::ContractKind;
    println!();
    let max_name = state.project.contracts.iter()
        .filter(|c| !c.name.is_empty())
        .map(|c| c.name.chars().count())
        .max().unwrap_or(0);

    for c in &state.project.contracts {
        if c.name.is_empty() { continue; }
        let badge = match c.kind {
            ContractKind::Contract => c_accent("[C]"),
            ContractKind::Interface => c_muted("[I]"),
            ContractKind::Library => c_muted("[L]"),
            ContractKind::Abstract => c_warn("[A]"),
        };
        let marker = if c.name == current {
            c_ok(" ← current").to_string()
        } else {
            String::new()
        };
        let padded = fmt::pad_right(&c.name, max_name);
        let details = format!(
            "{} functions, {} state vars",
            c.functions.iter().filter(|f| !f.name.is_empty()).count(),
            c.state_vars.len(),
        );
        let inherits = if c.inherits.is_empty() {
            String::new()
        } else {
            format!(", inherits {}", c.inherits.join(", "))
        };
        println!("  {} {}  {}{}{}", badge, c_accent(&padded), c_muted(&details), c_muted(&inherits), marker);
    }
    println!();
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
            println!("  {} Step {}: {} {} {}", c_ok("+"), step_index, c_bright(function), badge, format_access_detail(access));
            if !state_changed.is_empty() {
                println!("    {}:", c_muted("State writes"));
                for var in state_changed {
                    println!("      {} {}", c_muted("·"), c_warn(var));
                }
            }
            let seq_str = steps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ");
            println!("    {}: {}", c_muted("Sequence"), c_accent(&seq_str));
            println!();
        }
        CommandResult::StepRemoved { remaining } => {
            println!();
            println!("  {} Step removed. {} remaining.", c_warn("-"), remaining);
            if !steps.is_empty() {
                let seq_str = steps.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ");
                println!("    {}: {}", c_muted("Sequence"), c_accent(&seq_str));
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
            println!("{}", fmt::separator("STATE"));
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
            let max_name = functions.iter()
                .filter(|f| !f.name.is_empty())
                .map(|f| f.name.chars().count())
                .max().unwrap_or(0);
            for entry in functions {
                if entry.name.is_empty() { continue; }
                let badge = access_colored(&entry.access);
                let padded_name = fmt::pad_right(&entry.name, max_name);
                let mut tags: Vec<&str> = Vec::new();
                if entry.writes_state { tags.push("writes state"); }
                if entry.has_external_calls { tags.push("external calls"); }
                if entry.is_read_only { tags.push("view"); }
                let tag_str = if tags.is_empty() {
                    String::new()
                } else {
                    format!("  {}", c_muted(&tags.join(", ")))
                };
                println!("  {} {}{}", badge, c_accent(&padded_name), tag_str);
            }
            println!();
        }
        CommandResult::FunctionListAll { functions } => {
            println!();
            let max_name = functions.iter()
                .filter(|f| !f.name.is_empty())
                .map(|f| f.name.chars().count())
                .max().unwrap_or(0);

            let own: Vec<_> = functions.iter().filter(|f| !f.is_inherited).collect();
            let inherited: Vec<_> = functions.iter().filter(|f| f.is_inherited).collect();

            for entry in &own {
                if entry.name.is_empty() { continue; }
                let badge = access_colored(&entry.access);
                let padded_name = fmt::pad_right(&entry.name, max_name);
                let mut tags: Vec<&str> = Vec::new();
                if entry.writes_state { tags.push("writes state"); }
                if entry.has_external_calls { tags.push("external calls"); }
                if entry.is_read_only { tags.push("view"); }
                let tag_str = if tags.is_empty() {
                    String::new()
                } else {
                    format!("  {}", c_muted(&tags.join(", ")))
                };
                println!("  {} {}{}", badge, c_accent(&padded_name), tag_str);
            }

            if !inherited.is_empty() {
                println!();
                println!("  {}", c_muted("inherited:"));
                for entry in &inherited {
                    let badge = access_colored(&entry.access);
                    let padded_name = fmt::pad_right(&entry.name, max_name);
                    let origin = format!("from {}", entry.origin);
                    println!("  {} {}  {}", badge, c_muted(&padded_name), c_muted(&origin));
                }
            }
            println!();
        }
        CommandResult::StateVarListAll { state_vars } => {
            println!();
            let max_name = state_vars.iter()
                .map(|v| v.name.chars().count())
                .max().unwrap_or(0);
            let max_tag = 9;

            let own: Vec<_> = state_vars.iter().filter(|v| !v.is_inherited).collect();
            let inherited: Vec<_> = state_vars.iter().filter(|v| v.is_inherited).collect();

            let render_tag = |is_const: bool, is_immut: bool| -> String {
                let text = if is_const { "const" }
                    else if is_immut { "immutable" }
                    else { "mutable" };
                let padded = fmt::pad_right(text, max_tag);
                if is_const || is_immut {
                    c_muted(&padded).to_string()
                } else {
                    c_warn(&padded).to_string()
                }
            };

            for entry in &own {
                let tag = render_tag(entry.is_constant, entry.is_immutable);
                let padded_name = fmt::pad_right(&entry.name, max_name);
                println!("  {} {}  {}", tag, c_accent(&padded_name), c_muted(&entry.type_name));
            }

            if !inherited.is_empty() {
                println!();
                println!("  {}", c_muted("inherited:"));
                for entry in &inherited {
                    let tag = render_tag(entry.is_constant, entry.is_immutable);
                    let padded_name = fmt::pad_right(&entry.name, max_name);
                    let origin = format!("from {}", entry.origin);
                    println!("  {} {}  {}  {}",
                        tag,
                        c_muted(&padded_name),
                        c_muted(&entry.type_name),
                        c_muted(&origin),
                    );
                }
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
        CommandResult::Exported { markdown } => {
            println!("  {} chars exported", markdown.len());
        }
        CommandResult::SessionSaved { json } => {
            println!("  Session serialized ({} bytes)", json.len());
        }
        CommandResult::SessionLoaded { contract, steps: loaded_steps } => {
            println!("  Session loaded: {} ({} steps)", contract, loaded_steps.len());
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
    let max_name = vars.iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
        .map(|n| n.chars().count())
        .max().unwrap_or(0);
    let max_tag = 9; // "immutable" is the longest
    println!();
    for v in vars {
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let type_name = v.get("type_name").and_then(|n| n.as_str()).unwrap_or("?");
        let is_const = v.get("is_constant").and_then(|n| n.as_bool()).unwrap_or(false);
        let is_immut = v.get("is_immutable").and_then(|n| n.as_bool()).unwrap_or(false);

        let tag_text = if is_const { "const" }
            else if is_immut { "immutable" }
            else { "mutable" };
        let padded_tag = fmt::pad_right(tag_text, max_tag);
        let tag = if is_const || is_immut {
            c_muted(&padded_tag).to_string()
        } else {
            c_warn(&padded_tag).to_string()
        };

        let padded_name = fmt::pad_right(name, max_name);
        println!("  {} {}  {}", tag, c_accent(&padded_name), c_muted(type_name));
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

    // Build the list of sections that will be shown so we know which is last
    // (for picking the trailing branch character).
    #[derive(Default)]
    struct TransitiveGroup {
        writes: Vec<String>,
        reads: Vec<String>,
        external: Vec<String>,
        events: Vec<String>,
    }

    enum Section<'a> {
        Paths { total: u64, happy: u64, revert: u64 },
        StringList { label: &'a str, label_color: SectionColor, items: Vec<String> },
        Transitive(Vec<(String, TransitiveGroup)>),
        Observations(Vec<String>),
    }
    enum SectionColor { Muted, Danger, Warn, Accent }

    let color = |c: &SectionColor, s: &str| -> String {
        match c {
            SectionColor::Muted => c_muted(s).to_string(),
            SectionColor::Danger => c_danger(s).to_string(),
            SectionColor::Warn => c_warn(s).to_string(),
            SectionColor::Accent => c_accent(s).to_string(),
        }
    };

    let mut sections: Vec<Section> = Vec::new();

    if let Some(total) = val.get("total_paths").and_then(|v| v.as_u64()) {
        let happy = val.get("happy_paths").and_then(|v| v.as_u64()).unwrap_or(0);
        let revert = val.get("revert_paths").and_then(|v| v.as_u64()).unwrap_or(0);
        sections.push(Section::Paths { total, happy, revert });
    }

    let collect_strs = |key: &str| -> Vec<String> {
        val.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default()
    };

    let reads = collect_strs("state_reads");
    if !reads.is_empty() {
        sections.push(Section::StringList { label: "State reads", label_color: SectionColor::Muted, items: reads });
    }
    let writes = collect_strs("state_writes");
    if !writes.is_empty() {
        sections.push(Section::StringList { label: "State writes", label_color: SectionColor::Danger, items: writes });
    }
    let internal = collect_strs("internal_calls");
    if !internal.is_empty() {
        sections.push(Section::StringList { label: "Internal calls", label_color: SectionColor::Accent, items: internal });
    }
    let externals = collect_strs("external_calls");
    if !externals.is_empty() {
        sections.push(Section::StringList { label: "External calls", label_color: SectionColor::Warn, items: externals });
    }
    let events = collect_strs("events");
    if !events.is_empty() {
        sections.push(Section::StringList { label: "Events", label_color: SectionColor::Accent, items: events });
    }

    // Transitive effects (grouped by chain)
    let collect_transitive = |key: &str| -> Vec<(Vec<String>, String)> {
        val.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let via = e.get("via")?.as_array()?
                            .iter().filter_map(|s| s.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>();
                        let item = e.get("item")?.as_str()?.to_string();
                        Some((via, item))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let t_writes = collect_transitive("transitive_state_writes");
    let t_reads = collect_transitive("transitive_state_reads");
    let t_external = collect_transitive("transitive_external_calls");
    let t_events = collect_transitive("transitive_events");

    if !t_writes.is_empty() || !t_reads.is_empty() || !t_external.is_empty() || !t_events.is_empty() {
        use std::collections::BTreeMap;
        let mut groups: BTreeMap<String, TransitiveGroup> = BTreeMap::new();
        let join_chain = |via: &[String]| via.join(" → ");
        for (via, item) in t_writes { groups.entry(join_chain(&via)).or_default().writes.push(item); }
        for (via, item) in t_reads { groups.entry(join_chain(&via)).or_default().reads.push(item); }
        for (via, item) in t_external { groups.entry(join_chain(&via)).or_default().external.push(item); }
        for (via, item) in t_events { groups.entry(join_chain(&via)).or_default().events.push(item); }
        let ordered: Vec<(String, TransitiveGroup)> = groups.into_iter().collect();
        sections.push(Section::Transitive(ordered));
    }

    let obs_items: Vec<String> = val
        .get("observations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|o| o.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !obs_items.is_empty() {
        sections.push(Section::Observations(obs_items));
    }

    let total = sections.len();
    for (i, section) in sections.iter().enumerate() {
        let last = i == total - 1;
        let branch = if last { "└──" } else { "├──" };
        let cont = if last { "   " } else { "│  " };
        match section {
            Section::Paths { total, happy, revert } => {
                println!(
                    "  {} {} path(s): {} happy, {} revert",
                    c_muted(branch), total, c_ok(&happy.to_string()), c_danger(&revert.to_string())
                );
            }
            Section::StringList { label, label_color, items } => {
                println!("  {} {}:", c_muted(branch), color(label_color, label));
                for (j, item) in items.iter().enumerate() {
                    let leaf = if j == items.len() - 1 { "└──" } else { "├──" };
                    println!("  {}    {} {}", c_muted(cont), c_muted(leaf), c_muted(item));
                }
            }
            Section::Transitive(groups) => {
                println!("  {} {}:", c_muted(branch), c_warn("Transitive effects"));
                let gtotal = groups.len();
                for (gi, (chain, g)) in groups.iter().enumerate() {
                    let glast = gi == gtotal - 1;
                    let gbranch = if glast { "└──" } else { "├──" };
                    let gcont = if glast { "   " } else { "│  " };
                    println!("  {}    {} {} {}", c_muted(cont), c_muted(gbranch), c_muted("via"), c_muted(chain));

                    let mut parts: Vec<(&str, &Vec<String>)> = Vec::new();
                    if !g.writes.is_empty() { parts.push(("writes", &g.writes)); }
                    if !g.reads.is_empty() { parts.push(("reads", &g.reads)); }
                    if !g.external.is_empty() { parts.push(("external", &g.external)); }
                    if !g.events.is_empty() { parts.push(("emits", &g.events)); }
                    let ptotal = parts.len();
                    for (pi, (plabel, pitems)) in parts.iter().enumerate() {
                        let plast = pi == ptotal - 1;
                        let pbranch = if plast { "└──" } else { "├──" };
                        println!(
                            "  {}    {}    {} {}: {}",
                            c_muted(cont), c_muted(gcont), c_muted(pbranch),
                            c_muted(plabel), c_muted(&pitems.join(", "))
                        );
                    }
                }
            }
            Section::Observations(items) => {
                println!("  {} {}:", c_muted(branch), c_danger("Observations"));
                for (j, item) in items.iter().enumerate() {
                    let leaf = if j == items.len() - 1 { "└──" } else { "├──" };
                    println!("  {}    {} {}", c_muted(cont), c_muted(leaf), c_danger(item));
                }
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
    let cmds: &[(&str, &str, &str)] = &[
        ("c",      "call <func>",      "Add function to sequence"),
        ("b",      "back",             "Remove last step"),
        ("cl",     "clear",            "Reset sequence"),
        ("s",      "state",            "Show accumulated state"),
        ("f",      "functions",        "List available functions"),
        ("fa",     "funcs-all",        "List all accessible (incl. inherited)"),
        ("v",      "vars",             "List state variables"),
        ("va",     "vars-all",         "List all accessible (incl. inherited)"),
        ("ct",     "contracts",        "List project contracts"),
        ("",       "use <contract>",   "Switch active contract"),
        ("w",      "who <var>",        "Who reads/writes a variable"),
        ("i",      "info <func>",      "Function detail (no sequence change)"),
        ("seq",    "sequence",         "Sequence narrative with dependencies"),
        ("st",     "step <index>",     "Re-inspect a specific step"),
        ("ss",     "session",          "Full session state"),
        ("fi",     "finding [sev] [t]","Record a finding"),
        ("n",      "note <text>",      "Add note to current step"),
        ("sc",     "scenario <name>",  "Name the current sequence"),
        ("",       "status <f> <s>",   "Change review status"),
        ("fl",     "findings",         "List recorded findings"),
        ("ex",     "export",           "Export findings as markdown"),
        ("",       "save <name>",      "Save session to disk"),
        ("",       "load <name>",      "Load session from disk"),
        ("",       "browser",          "Open web UI"),
        ("q",      "quit/exit",        "Exit"),
    ];

    println!();
    println!("  {}  Commands:", c_bright("ilold explore"));
    println!();
    for (shortcut, name, desc) in cmds {
        let sc = if shortcut.is_empty() {
            format!("  {}  ", fmt::pad_right("", 3))
        } else {
            format!("  {} {}", c_accent(&fmt::pad_right(shortcut, 3)), c_muted("|"))
        };
        println!("  {} {}  {}", sc, c_accent(&fmt::pad_right(name, 18)), c_muted(desc));
    }
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
    contracts: Vec<String>,
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

        let needs_contract = line_lower.starts_with("use ");

        if !needs_func && !needs_contract {
            return Vec::new();
        }

        let arg_start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let partial = &line[arg_start..pos];

        let source = if needs_contract { &self.contracts } else { &self.functions };

        source.iter()
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
