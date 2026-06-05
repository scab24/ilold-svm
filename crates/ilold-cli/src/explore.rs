use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::Result;
use reedline::{
    Completer, DefaultHinter, FileBackedHistory, Prompt, PromptEditMode,
    PromptHistorySearch, PromptHistorySearchStatus, Reedline, Signal, Span, Suggestion,
};

use ilold_solana_core::exploration::SolanaCommandResult;
use ilold_solana_core::view::ProgramView;

use crate::colors::*;

pub async fn run(
    _paths: Vec<PathBuf>,
    _port: u16,
    _max_seq_depth: usize,
    attach: Option<String>,
) -> Result<()> {
    let url = attach.ok_or_else(|| {
        anyhow::anyhow!("explore::run requires --attach <url>; use explore::run_solana for local projects")
    })?;
    let client = reqwest::Client::new();
    let map_resp = client
        .get(format!("{url}/api/project/map"))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Cannot reach server at {url}: {e}"))?;
    if !map_resp.status().is_success() {
        anyhow::bail!("Server at {url} returned {}", map_resp.status());
    }
    let project_map: serde_json::Value = map_resp.json().await?;
    let kind = project_map["kind"].as_str().unwrap_or("solana");
    if kind != "solana" {
        anyhow::bail!("Only Solana servers are supported (got kind={kind})");
    }
    run_solana_attach(url, client, project_map).await
}

async fn run_solana_attach(
    url: String,
    _client: reqwest::Client,
    project_map: serde_json::Value,
) -> Result<()> {
    let programs_arr = project_map["programs"].as_array();
    let program_name = programs_arr
        .and_then(|arr| arr.first())
        .and_then(|p| p["name"].as_str())
        .unwrap_or("unknown")
        .to_string();
    let program_names: Vec<String> = programs_arr
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let function_names: Vec<String> = programs_arr
        .and_then(|arr| arr.iter().find(|p| p["name"].as_str() == Some(&program_name)))
        .and_then(|p| p["instructions"].as_array())
        .map(|ixs| {
            ixs.iter()
                .filter_map(|i| i["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let banner = header_box(&[
        &format!("ilold explore — {} (solana, attached)", program_name),
        &format!("{} instructions | Type ? for help", function_names.len()),
        &format!("Server: {}", url),
    ]);
    println!("{}\n", banner);

    let handle = tokio::runtime::Handle::current();
    let repl_thread = std::thread::spawn(move || {
        repl_loop(handle, program_name, function_names, program_names, None, url);
    });
    repl_thread
        .join()
        .map_err(|_| anyhow::anyhow!("REPL thread panicked"))?;
    Ok(())
}

pub async fn run_solana(
    detected: ilold_solana_core::ingest::DetectedProject,
    port: u16,
) -> Result<()> {
    println!("Analyzing {} IDL(s)...", detected.idl_paths.len());
    let (state, actual_port) = ilold_web::start_solana_server(detected, port).await?;
    run_with_state(state, actual_port).await
}

async fn run_with_state(
    state: std::sync::Arc<ilold_web::state::AppState>,
    actual_port: u16,
) -> Result<()> {
    let s = state.solana().expect("solana backend required");
    let program = s.project.programs.first();
    let program_name = program
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "unknown".into());
    let function_names: Vec<String> = program
        .map(|p| p.instructions.iter().map(|i| i.name.clone()).collect())
        .unwrap_or_default();
    let program_names: Vec<String> = s
        .project
        .programs
        .iter()
        .map(|p| p.name.clone())
        .collect();

    let banner = header_box(&[
        &format!("ilold explore — {} (solana)", program_name),
        &format!("{} instructions | Type ? for help", function_names.len()),
        &format!("Web UI: http://localhost:{}", actual_port),
    ]);
    println!("{}\n", banner);

    let handle = tokio::runtime::Handle::current();
    let state_for_thread = state.clone();
    let base_url = format!("http://127.0.0.1:{}", actual_port);
    let repl_thread = std::thread::spawn(move || {
        repl_loop(
            handle,
            program_name,
            function_names,
            program_names,
            Some(state_for_thread),
            base_url,
        );
    });

    repl_thread
        .join()
        .map_err(|_| anyhow::anyhow!("REPL thread panicked"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn repl_loop(
    handle: tokio::runtime::Handle,
    mut contract: String,
    mut functions: Vec<String>,
    program_names: Vec<String>,
    state: Option<std::sync::Arc<ilold_web::state::AppState>>,
    base_url: String,
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
        contracts: program_names.clone(),
        scenarios: vec!["main".to_string()],
    }));

    let mut editor = Reedline::create()
        .with_history(history)
        .with_completer(Box::new(CompleterWrapper(completer.clone())))
        .with_hinter(Box::new(DefaultHinter::default().with_style(
            nu_ansi_term::Style::new().fg(nu_ansi_term::Color::DarkGray),
        )));

    let client = reqwest::Client::new();
    let mut steps: Vec<String> = Vec::new();
    let mut scenario_name: String = "main".into();

    let mut prompt = IloldPrompt {
        contract: contract.clone(),
        steps: Vec::new(),
        scenario: scenario_name.clone(),
    };

    let attached = state.is_none();
    if attached {
        if let Some(server_steps) = sync_steps(&handle, &client, &base_url, &contract) {
            steps = server_steps;
            prompt.steps = steps.clone();
        }
        if let Some(active) = sync_active_scenario(&handle, &client, &base_url, &contract) {
            scenario_name = active;
            prompt.scenario = scenario_name.clone();
        }
        if let Some(scns) = sync_scenarios(&handle, &client, &base_url, &contract) {
            if let Ok(mut comp) = completer.lock() {
                comp.scenarios = scns;
            }
        }
    }

    loop {
        if attached {
            if let Some(server_steps) = sync_steps(&handle, &client, &base_url, &contract) {
                if server_steps != steps {
                    steps = server_steps;
                    prompt.steps = steps.clone();
                }
            }
            if let Some(active) = sync_active_scenario(&handle, &client, &base_url, &contract) {
                if active != scenario_name {
                    scenario_name = active;
                    prompt.scenario = scenario_name.clone();
                }
            }
            if let Some(scns) = sync_scenarios(&handle, &client, &base_url, &contract) {
                if let Ok(mut comp) = completer.lock() {
                    if comp.scenarios != scns {
                        comp.scenarios = scns;
                    }
                }
            }
        }

        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                match handle_solana_input(
                    line,
                    &handle,
                    &client,
                    &base_url,
                    &contract,
                    &mut steps,
                    &mut scenario_name,
                    &completer,
                ) {
                    InputResult::Continue => {}
                    InputResult::Quit => break,
                    InputResult::UpdatePrompt => {
                        prompt.steps = steps.clone();
                        prompt.scenario = scenario_name.clone();
                    }
                    InputResult::SwitchContract(new_name) => {
                        contract = new_name.clone();
                        steps.clear();
                        scenario_name = "main".into();
                        if let Some(state) = state.as_ref() {
                            if let Some(s) = state.solana() {
                                if let Some(p) =
                                    s.project.programs.iter().find(|p| p.name == new_name)
                                {
                                    functions =
                                        p.instructions.iter().map(|i| i.name.clone()).collect();
                                    if let Ok(mut comp) = completer.lock() {
                                        comp.functions = functions.clone();
                                    }
                                }
                            }
                        }
                        prompt.contract = contract.clone();
                        prompt.steps = Vec::new();
                        prompt.scenario = scenario_name.clone();
                        if let Ok(mut comp) = completer.lock() {
                            comp.scenarios = vec!["main".to_string()];
                        }
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
        self.0
            .lock()
            .map(|mut c| c.complete(line, pos))
            .unwrap_or_default()
    }
}

fn inline_help_target(cmd: &str, arg: &str) -> Option<String> {
    if cmd == "?" || cmd == "help" || cmd == "h" {
        return None;
    }
    if cmd.ends_with('?') && cmd.len() > 1 {
        return Some(cmd[..cmd.len() - 1].to_string());
    }
    if arg.trim() == "?" {
        return Some(cmd.to_string());
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn handle_solana_input(
    line: &str,
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
    steps: &mut Vec<String>,
    scenario_name: &mut String,
    completer: &std::sync::Arc<std::sync::Mutex<IloldCompleter>>,
) -> InputResult {
    let normalized = split_numeric_suffix(line);
    let parts: Vec<&str> = normalized.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

    if let Some(base) = inline_help_target(&cmd, arg) {
        match crate::help::render_solana_help_block(&base) {
            Some(text) => print!("{text}"),
            None => println!(
                "  {} no help registered for {}",
                c_danger("✗"),
                c_accent(&base)
            ),
        }
        return InputResult::Continue;
    }

    match cmd.as_str() {
        "?" | "help" | "h" => {
            print_solana_help();
            InputResult::Continue
        }
        "quit" | "q" | "exit" => InputResult::Quit,
        "funcs" | "functions" | "f" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Funcs"),
            steps,
        ),
        "funcs-all" | "fa" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Funcs"),
            steps,
        ),
        "info" | "i" => {
            if arg.is_empty() {
                println!("  Usage: info <instruction>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({"Info": {"ix": arg}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "vars" | "v" | "vars-all" | "va" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Vars"),
            steps,
        ),
        "coupling" | "cp" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Coupling"),
            steps,
        ),
        "coverage" | "cov" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Coverage"),
            steps,
        ),
        "state" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("State"),
            steps,
        ),
        "session" | "s" => {
            let r = dispatch_solana(
                handle,
                client,
                base_url,
                contract,
                serde_json::json!("Session"),
                steps,
            );
            *scenario_name = sync_active_scenario(handle, client, base_url, contract)
                .unwrap_or_else(|| scenario_name.clone());
            r
        }
        "back" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Back"),
            steps,
        ),
        "clear" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Clear"),
            steps,
        ),
        "users" => {
            if arg.starts_with("new") {
                let rest = arg.trim_start_matches("new").trim();
                if rest.is_empty() {
                    println!("  Usage: users new <name> [<lamports>]");
                    return InputResult::Continue;
                }
                let parts: Vec<&str> = rest.split_whitespace().collect();
                let name = parts[0].to_string();
                let lamports: u64 = parts
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10_000_000_000);
                let body = serde_json::json!({"UsersNew": {"name": name, "lamports": lamports}});
                dispatch_solana(handle, client, base_url, contract, body, steps)
            } else {
                dispatch_solana(
                    handle,
                    client,
                    base_url,
                    contract,
                    serde_json::json!("Users"),
                    steps,
                )
            }
        }
        "airdrop" | "air" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.len() != 2 {
                println!("  Usage: airdrop <name> <lamports>");
                return InputResult::Continue;
            }
            let lamports: u64 = match parts[1].parse() {
                Ok(v) => v,
                Err(_) => {
                    println!("  Lamports must be an integer");
                    return InputResult::Continue;
                }
            };
            let body =
                serde_json::json!({"Airdrop": {"user": parts[0], "lamports": lamports}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "time-warp" | "tw" => {
            let delta: i64 = match arg.parse() {
                Ok(v) => v,
                Err(_) => {
                    println!("  Usage: time-warp <delta_seconds>");
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({"TimeWarp": {"delta_seconds": delta}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "pda" => {
            if arg.is_empty() {
                println!("  Usage: pda <instruction>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({"Pda": {"instruction": arg}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "inspect" | "acc" => {
            if arg.is_empty() {
                println!("  Usage: inspect <pubkey>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({"Inspect": {"pubkey": arg}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "call" | "c" => {
            let parts: Vec<&str> = arg.splitn(2, ' ').collect();
            if parts.is_empty() || parts[0].is_empty() {
                println!("  Usage: call <ix> [arg=val ...] [account=user_or_pubkey ...]");
                println!("         or: call <ix> {{json}} for full control");
                return InputResult::Continue;
            }
            let ix = parts[0].to_string();
            let payload_raw = parts.get(1).copied().unwrap_or("").trim();
            let body = if payload_raw.starts_with('{') {
                let parsed: serde_json::Value = match serde_json::from_str(payload_raw) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("  Invalid JSON: {e}");
                        return InputResult::Continue;
                    }
                };
                serde_json::json!({
                    "Call": {
                        "ix": ix,
                        "args": parsed.get("args").cloned().unwrap_or(serde_json::json!({})),
                        "accounts": parsed.get("accounts").cloned().unwrap_or(serde_json::json!({})),
                        "signers": parsed.get("signers").cloned().unwrap_or(serde_json::json!([])),
                    }
                })
            } else {
                let program = match fetch_program_detail(handle, client, base_url, contract) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("  {}", c_danger(&format!("fetch program: {e}")));
                        return InputResult::Continue;
                    }
                };
                match build_call_from_kv(&program, &ix, payload_raw) {
                    Ok(body) => body,
                    Err(e) => {
                        eprintln!("  {}", c_danger(&e));
                        return InputResult::Continue;
                    }
                }
            };
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "ct" | "contracts" | "programs" | "progs" => {
            match handle.block_on(async {
                client
                    .get(format!("{base_url}/api/project/map"))
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await
            }) {
                Ok(map) => print_remote_programs(&map, contract),
                Err(e) => {
                    eprintln!("  {}", c_danger(&format!("Failed to fetch programs: {e}")))
                }
            }
            InputResult::Continue
        }
        "use" => {
            if arg.is_empty() {
                println!("  Usage: use <program>");
                return InputResult::Continue;
            }
            InputResult::SwitchContract(arg.to_string())
        }
        "sc" | "scenario" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            let sub = parts.first().copied().unwrap_or("");
            let name_arg = parts.get(1).copied().unwrap_or("");
            let action: Option<serde_json::Value> = match sub {
                "new" if !name_arg.is_empty() => {
                    Some(serde_json::json!({"New": {"name": name_arg}}))
                }
                "list" | "ls" | "" => Some(serde_json::json!("List")),
                "switch" if !name_arg.is_empty() => {
                    Some(serde_json::json!({"Switch": {"name": name_arg}}))
                }
                "fork" if !name_arg.is_empty() => {
                    let at_step: Option<usize> = parts.get(2).and_then(|s| s.parse().ok());
                    Some(serde_json::json!({"Fork": {"name": name_arg, "at_step": at_step}}))
                }
                "delete" | "rm" if !name_arg.is_empty() => {
                    Some(serde_json::json!({"Delete": {"name": name_arg}}))
                }
                _ => None,
            };
            let action = match action {
                Some(a) => a,
                None => {
                    println!("  Usage: scenario new|list|switch|fork|delete <name> [step]");
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({"Scenario": {"sub": action}});
            let outcome = dispatch_solana(handle, client, base_url, contract, body, steps);
            *scenario_name = sync_active_scenario(handle, client, base_url, contract)
                .unwrap_or_else(|| scenario_name.clone());
            if let Ok(mut comp) = completer.lock() {
                if let Some(items) = sync_scenarios(handle, client, base_url, contract) {
                    comp.scenarios = items;
                }
            }
            outcome
        }
        "note" | "n" => {
            if arg.is_empty() {
                println!("  Usage: note <text>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({"Note": {"text": arg}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "fi" | "finding" => {
            if arg.is_empty() {
                println!("  Usage: finding <severity> <title> [--rec=\"...\"]");
                println!("  Severity: critical | high | medium | low | info");
                return InputResult::Continue;
            }
            let (rest, rec): (&str, Option<String>) = match arg.find("--rec=") {
                Some(idx) => {
                    let head = arg[..idx].trim_end();
                    let tail = &arg[idx + "--rec=".len()..];
                    (head, Some(strip_quotes(tail).to_string()))
                }
                None => (arg, None),
            };
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() < 2 {
                println!("  Usage: finding <severity> <title> [--rec=\"...\"]");
                return InputResult::Continue;
            }
            let severity = match normalize_severity(parts[0]) {
                Some(s) => s,
                None => {
                    println!(
                        "  {}",
                        c_danger("Invalid severity. Valid: critical, high, medium, low, info")
                    );
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({
                "Finding": {
                    "severity": severity,
                    "title": parts[1],
                    "description": "",
                    "recommendation": rec,
                }
            });
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "seq" | "sequence" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Session"),
            steps,
        ),
        "browser" => {
            println!("  {} Web UI not yet available in explore mode.", c_muted("·"));
            println!("  {} API running at {}/api/", c_muted("·"), base_url);
            InputResult::Continue
        }
        "step" | "st" => {
            let idx: usize = match arg.trim().parse() {
                Ok(n) => n,
                Err(_) => {
                    println!("  Usage: step <index>");
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({"Step": {"index": idx}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "save" => {
            if arg.is_empty() {
                println!("  Usage: save <name> [--with-keypairs]");
                return InputResult::Continue;
            }
            let mut with_keypairs = false;
            let mut name: Option<&str> = None;
            for tok in arg.split_whitespace() {
                if tok == "--with-keypairs" {
                    with_keypairs = true;
                } else if tok.starts_with("--") {
                    println!("  Unknown flag: {tok}. Use --with-keypairs (or no flags).");
                    return InputResult::Continue;
                } else if name.is_none() {
                    name = Some(tok);
                } else {
                    println!("  Usage: save <name> [--with-keypairs]");
                    return InputResult::Continue;
                }
            }
            let name = match name {
                Some(n) => n,
                None => {
                    println!("  Usage: save <name> [--with-keypairs]");
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({
                "contract": contract,
                "command": {"SaveSession": {"with_keypairs": with_keypairs}},
            });
            match send_solana_command(handle, client, base_url, &body) {
                Ok(SolanaCommandResult::SessionSaved { json }) => {
                    let dir = dirs::home_dir()
                        .map(|h| h.join(".ilold").join("sessions"))
                        .unwrap_or_else(|| std::path::PathBuf::from(".ilold/sessions"));
                    std::fs::create_dir_all(&dir).ok();
                    let path = dir.join(format!("{}.json", name));
                    match std::fs::write(&path, &json) {
                        Ok(_) => {
                            println!(
                                "  {} Saved to {}",
                                c_ok("✓"),
                                c_accent(&path.display().to_string())
                            );
                            if with_keypairs {
                                eprintln!(
                                    "  {} bundle includes plaintext test keypairs — do NOT commit it",
                                    c_warn("⚠ "),
                                );
                            }
                        }
                        Err(e) => eprintln!("  {} Write failed: {}", c_danger("✗"), e),
                    }
                }
                Ok(other) => print_solana_result(&other),
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
                    eprintln!(
                        "  {} File not found: {} ({})",
                        c_danger("✗"),
                        path.display(),
                        e
                    );
                    return InputResult::Continue;
                }
            };
            if json.contains("\"keypairs_present\": true")
                || json.contains("\"keypairs_present\":true")
            {
                eprintln!(
                    "  {} bundle contains plaintext test keypairs — do NOT commit *.json files like this",
                    c_warn("⚠ "),
                );
            }
            let body = serde_json::json!({
                "contract": contract,
                "command": {"LoadSession": {"json": json}}
            });
            match send_solana_command(handle, client, base_url, &body) {
                Ok(SolanaCommandResult::SessionLoaded { steps: loaded, .. }) => {
                    *steps = loaded;
                    println!("  {} Session loaded ({} steps)", c_ok("✓"), steps.len());
                    return InputResult::UpdatePrompt;
                }
                Ok(other) => print_solana_result(&other),
                Err(e) => eprintln!("  {}", c_danger(&e)),
            }
            InputResult::Continue
        }
        "findings" | "fl" => dispatch_solana(
            handle,
            client,
            base_url,
            contract,
            serde_json::json!("Findings"),
            steps,
        ),
        "export" | "ex" => {
            let mut auditor: Option<String> = None;
            let mut version: Option<String> = None;
            let mut date: Option<String> = None;
            for tok in arg.split_whitespace() {
                if let Some(v) = tok.strip_prefix("--auditor=") {
                    auditor = Some(strip_quotes(v).to_string());
                } else if let Some(v) = tok.strip_prefix("--version=") {
                    version = Some(strip_quotes(v).to_string());
                } else if let Some(v) = tok.strip_prefix("--date=") {
                    date = Some(strip_quotes(v).to_string());
                } else {
                    println!(
                        "  Unknown flag: {tok}. Use --auditor= / --version= / --date= (or no flags)"
                    );
                    return InputResult::Continue;
                }
            }
            let metadata = if auditor.is_some() || version.is_some() || date.is_some() {
                Some(serde_json::json!({
                    "auditor": auditor,
                    "project_version": version,
                    "audit_date": date,
                }))
            } else {
                None
            };
            let body = serde_json::json!({"Export": {"metadata": metadata}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "who" => {
            if arg.is_empty() {
                println!("  Usage: who <account_type>  (e.g. who Pool)");
                return InputResult::Continue;
            }
            let body = serde_json::json!({"Who": {"account_type": arg}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "timeline" | "tl" => {
            if arg.is_empty() {
                println!("  Usage: timeline <pubkey>");
                return InputResult::Continue;
            }
            let body = serde_json::json!({"Timeline": {"pubkey": arg}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        "status" => {
            let parts: Vec<&str> = arg.split_whitespace().collect();
            if parts.len() != 2 {
                println!("  Usage: status <ix> <open|reviewed|finding>");
                return InputResult::Continue;
            }
            let st = match parts[1].to_lowercase().as_str() {
                "open" => "Open",
                "reviewed" => "Reviewed",
                "finding" | "found" => "Finding",
                other => {
                    println!("  Unknown status '{other}'. Use open|reviewed|finding");
                    return InputResult::Continue;
                }
            };
            let body = serde_json::json!({"Status": {"ix": parts[0], "status": st}});
            dispatch_solana(handle, client, base_url, contract, body, steps)
        }
        _ => {
            println!(
                "  Unknown command: {}. Type {} for help.",
                c_danger(&cmd),
                c_accent("?")
            );
            InputResult::Continue
        }
    }
}

fn dispatch_solana(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
    command: serde_json::Value,
    steps: &mut Vec<String>,
) -> InputResult {
    let body = serde_json::json!({"contract": contract, "command": command});
    match send_solana_command(handle, client, base_url, &body) {
        Ok(result) => {
            apply_solana_result_to_steps(&result, steps);
            print_solana_result(&result);
            InputResult::UpdatePrompt
        }
        Err(e) => {
            eprintln!("  {}", c_danger(&e));
            InputResult::Continue
        }
    }
}

fn fetch_program_detail(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    name: &str,
) -> Result<ProgramView, String> {
    handle.block_on(async {
        let resp = client
            .get(format!("{base_url}/api/program/{name}/view"))
            .send()
            .await
            .map_err(|e| format!("request: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("status {}", resp.status()));
        }
        resp.json::<ProgramView>()
            .await
            .map_err(|e| format!("parse: {e}"))
    })
}

fn build_call_from_kv(
    program: &ProgramView,
    ix_name: &str,
    rest: &str,
) -> Result<serde_json::Value, String> {
    let ix = program
        .instructions
        .iter()
        .find(|i| i.name == ix_name)
        .ok_or_else(|| format!("instruction '{ix_name}' not found in program"))?;

    let mut args = serde_json::Map::new();
    let mut accounts = serde_json::Map::new();
    let mut signer_overrides: Option<Vec<String>> = None;
    let mut signer_negatives: Vec<String> = Vec::new();

    for token in rest.split_whitespace() {
        if let Some(name_csv) = token.strip_prefix("--no-signer=") {
            for n in name_csv.split(',').map(|s| s.trim()) {
                if !n.is_empty() {
                    signer_negatives.push(n.to_string());
                }
            }
            continue;
        }
        if let Some(name_csv) = token.strip_prefix("--signer=") {
            let mut acc = signer_overrides.unwrap_or_default();
            for n in name_csv.split(',').map(|s| s.trim()) {
                if !n.is_empty() {
                    acc.push(n.to_string());
                }
            }
            signer_overrides = Some(acc);
            continue;
        }
        let (key, value) = match token.split_once('=') {
            Some(kv) => kv,
            None => return Err(format!("expected key=value, got '{token}'")),
        };
        if let Some(arg) = ix.args.iter().find(|a| a.name == key) {
            args.insert(key.to_string(), coerce_kv(value, &arg.ty));
        } else if ix.accounts.iter().any(|a| a.name == key) {
            accounts.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        } else {
            let arg_list: Vec<String> = ix.args.iter().map(|a| a.name.clone()).collect();
            let acc_list: Vec<String> = ix.accounts.iter().map(|a| a.name.clone()).collect();
            return Err(format!(
                "unknown key '{key}'; expected one of args [{}] or accounts [{}]",
                arg_list.join(","),
                acc_list.join(",")
            ));
        }
    }

    let signers = match signer_overrides {
        Some(list) => list
            .into_iter()
            .filter(|n| !signer_negatives.contains(n))
            .collect(),
        None => ix
            .accounts
            .iter()
            .filter(|a| a.signer)
            .filter_map(|a| accounts.get(&a.name).and_then(|v| v.as_str()).map(String::from))
            .filter(|n| !signer_negatives.contains(n))
            .collect::<Vec<String>>(),
    };

    Ok(serde_json::json!({
        "Call": {
            "ix": ix_name,
            "args": serde_json::Value::Object(args),
            "accounts": serde_json::Value::Object(accounts),
            "signers": signers,
        }
    }))
}

fn coerce_kv(raw: &str, ty: &str) -> serde_json::Value {
    match ty {
        "bool" => return serde_json::Value::Bool(raw == "true" || raw == "1"),
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" => {
            if let Ok(n) = raw.parse::<u64>() {
                return serde_json::Value::Number(n.into());
            }
            if let Ok(n) = raw.parse::<i64>() {
                return serde_json::Value::Number(n.into());
            }
            if let Ok(f) = raw.parse::<f64>() {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    return serde_json::Value::Number(n);
                }
            }
        }
        _ => {}
    }
    serde_json::Value::String(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ilold_solana_core::idl::parse_idl;
    use ilold_solana_core::model::ProgramDef;

    const STAKING_IDL: &str = include_str!(
        "../../../tests/fixtures/solana/staking/idls/staking.json"
    );

    fn staking_view() -> ProgramView {
        ProgramDef::from_idl(parse_idl(STAKING_IDL).expect("parse staking idl"))
            .expect("build staking ProgramDef")
            .compute_view()
    }

    #[test]
    fn kv_parser_distributes_args_and_accounts() {
        let program = staking_view();
        let body = build_call_from_kv(
            &program,
            "initialize_pool",
            "reward_rate=10 pool=pool admin=admin",
        )
        .expect("kv parse");
        assert_eq!(body["Call"]["ix"], "initialize_pool");
        assert_eq!(body["Call"]["args"]["reward_rate"], 10);
        assert_eq!(body["Call"]["accounts"]["pool"], "pool");
        assert_eq!(body["Call"]["accounts"]["admin"], "admin");
        let signers: Vec<_> = body["Call"]["signers"].as_array().unwrap().iter().collect();
        assert!(signers.iter().any(|v| v.as_str() == Some("pool")));
        assert!(signers.iter().any(|v| v.as_str() == Some("admin")));
    }

    #[test]
    fn kv_parser_supports_no_signer_override() {
        let program = staking_view();
        let body = build_call_from_kv(
            &program,
            "initialize_pool",
            "reward_rate=10 pool=pool admin=admin --no-signer=admin",
        )
        .expect("kv parse");
        let signers: Vec<&str> = body["Call"]["signers"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(signers.contains(&"pool"));
        assert!(!signers.contains(&"admin"));
    }

    #[test]
    fn kv_parser_rejects_unknown_key() {
        let program = staking_view();
        let err = build_call_from_kv(
            &program,
            "initialize_pool",
            "reward_rate=10 ghost=foo",
        )
        .unwrap_err();
        assert!(err.contains("ghost"), "got: {err}");
    }

    #[test]
    fn kv_parser_omits_constant_accounts_from_form() {
        let program = staking_view();
        let body = build_call_from_kv(
            &program,
            "initialize_pool",
            "reward_rate=10 pool=pool admin=admin",
        )
        .expect("kv parse");
        assert!(body["Call"]["accounts"].get("system_program").is_none());
    }
}

fn send_solana_command(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    body: &serde_json::Value,
) -> Result<SolanaCommandResult, String> {
    handle.block_on(async {
        let resp = client
            .post(format!("{base_url}/api/cmd"))
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Server error {status}: {text}"));
        }
        resp.json::<SolanaCommandResult>()
            .await
            .map_err(|e| format!("Parse failed: {e}"))
    })
}

fn apply_solana_result_to_steps(result: &SolanaCommandResult, steps: &mut Vec<String>) {
    match result {
        SolanaCommandResult::StepAdded { instruction, .. } => steps.push(instruction.clone()),
        SolanaCommandResult::StepRemoved { .. } => {
            steps.pop();
        }
        SolanaCommandResult::Cleared => steps.clear(),
        SolanaCommandResult::SessionView { steps: server_steps, .. } => {
            *steps = server_steps.clone();
        }
        SolanaCommandResult::SessionLoaded { steps: server_steps, .. } => {
            *steps = server_steps.clone();
        }
        _ => {}
    }
}

fn print_solana_result(result: &SolanaCommandResult) {
    println!();
    if let SolanaCommandResult::Error { message } = result {
        eprintln!("  {} {}", c_danger("✗"), message);
        println!();
        return;
    }
    print!("{}", ilold_render::render_solana_result(result));
    println!();
}

fn sync_active_scenario(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "contract": contract,
        "command": {"Scenario": {"sub": "List"}}
    });
    let res = send_solana_command(handle, client, base_url, &body).ok()?;
    if let SolanaCommandResult::ScenarioList { items } = res {
        items.into_iter().find(|i| i.active).map(|i| i.name)
    } else {
        None
    }
}

fn sync_scenarios(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
) -> Option<Vec<String>> {
    let body = serde_json::json!({
        "contract": contract,
        "command": {"Scenario": {"sub": "List"}}
    });
    let res = send_solana_command(handle, client, base_url, &body).ok()?;
    if let SolanaCommandResult::ScenarioList { items } = res {
        Some(items.into_iter().map(|i| i.name).collect())
    } else {
        None
    }
}

fn sync_steps(
    handle: &tokio::runtime::Handle,
    client: &reqwest::Client,
    base_url: &str,
    contract: &str,
) -> Option<Vec<String>> {
    let body = serde_json::json!({
        "contract": contract,
        "command": "Session"
    });
    match send_solana_command(handle, client, base_url, &body) {
        Ok(SolanaCommandResult::SessionView { steps, .. }) => Some(steps),
        _ => None,
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

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        &s[1..s.len() - 1]
    } else {
        s
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

fn print_remote_programs(map: &serde_json::Value, current: &str) {
    let arr = match map.get("programs").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            println!("  {}", c_muted("No programs in /api/project/map"));
            return;
        }
    };
    println!();
    if arr.is_empty() {
        println!("  {}", c_muted("No programs detected"));
        println!();
        return;
    }
    for p in arr {
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let pid = p.get("program_id").and_then(|v| v.as_str()).unwrap_or("");
        let ix_count = p
            .get("instructions")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let marker = if name == current {
            c_ok(" ← current").to_string()
        } else {
            String::new()
        };
        println!(
            "  {} {} {} {}{}",
            c_accent("[P]"),
            name,
            c_muted(&format!("({} ix)", ix_count)),
            c_muted(pid),
            marker
        );
    }
    println!();
}

fn print_solana_help() {
    let groups: &[(&str, &[(&str, &str, &str)])] = &[
        ("Session", &[
            ("c",  "call <ix> arg=val acc=user", "Concise: keys auto-distributed; signers auto from IDL"),
            ("",   "call <ix> {json}",  "Full control with explicit args/accounts/signers"),
            ("",   "  --signer=a,b",    "Add signers (override IDL defaults)"),
            ("",   "  --no-signer=a",   "Remove a signer (simulate negative cases)"),
            ("b",  "back",              "Remove last step from active scenario"),
            ("cl", "clear",             "Reset active scenario steps"),
            ("",   "state",             "Decoded view of accounts mutated this session"),
            ("s",  "session",           "Active scenario summary (steps + findings)"),
        ]),
        ("Programs", &[
            ("ct", "programs (alias contracts)", "List programs in workspace"),
            ("",   "use <program>",     "Switch active program"),
            ("f",  "funcs (alias functions)", "List instructions of active program"),
            ("fa", "funcs-all",         "Instructions with arg/account/signer/pda counts"),
            ("i",  "info <ix>",         "Detail an instruction: args (typed), accounts (flags), discriminator"),
            ("v",  "vars",              "List declared account types with discriminators"),
            ("va", "vars-all",          "Account types with their decoded field layout"),
        ]),
        ("Solana runtime", &[
            ("",   "users",             "List keypairs in active scenario"),
            ("",   "users new <name> [lamports]", "Create keypair + airdrop (default 10 SOL)"),
            ("",   "airdrop <user> <lamports>", "Top up an existing keypair"),
            ("tw", "time-warp <secs>",  "Advance Clock unix_timestamp + slot"),
            ("",   "pda <ix>",          "List PDAs declared by an instruction (symbolic)"),
            ("",   "inspect <pubkey>",  "Read VM account, decode by Anchor discriminator"),
        ]),
        ("Analysis", &[
            ("st", "step <index>",      "Re-inspect a step: CU, logs, diffs"),
            ("",   "who <query>",        "Resolve query: AccountType | Instruction | Field"),
            ("tl", "timeline <pubkey>",  "Cross-step mutation history of an account, decoded"),
            ("cp", "coupling",           "List ix pairs that share writable accounts"),
            ("cov","coverage",           "Aggregated runtime metrics for the active scenario"),
        ]),
        ("Findings", &[
            ("fi", "finding <sev> <title>", "Record a security finding"),
            ("fl", "findings",          "List recorded findings"),
            ("n",  "note <text>",       "Add annotation to active sequence"),
            ("",   "status <ix> <s>",   "Set review status: open|reviewed|finding"),
            ("ex", "export",            "Markdown report: sequence + findings + program info"),
        ]),
        ("Workspace", &[
            ("sc", "scenario <sub>",    "new|list|switch|fork|delete <name> [step]"),
            ("",   "save <name>",       "Save active scenario JSON to disk"),
            ("",   "load <name>",       "Load scenario JSON from disk"),
            ("?",  "help",              "Print this help (append ? to any cmd for full reference)"),
            ("q",  "quit/exit",         "Exit"),
        ]),
    ];

    println!();
    println!("  {}  {}", c_bright("ilold explore"), c_muted("— append ? to any command for inline help (e.g. call?)"));
    println!();
    for (group_name, cmds) in groups {
        println!("  {}", c_warn(group_name));
        for (shortcut, name, desc) in *cmds {
            let sc = if shortcut.is_empty() {
                format!("  {}  ", pad_right("", 3))
            } else {
                format!("  {} {}", c_accent(&pad_right(shortcut, 3)), c_muted("|"))
            };
            println!("  {} {}  {}", sc, c_accent(&pad_right(name, 22)), c_muted(desc));
        }
        println!();
    }
}

struct IloldPrompt {
    contract: String,
    steps: Vec<String>,
    scenario: String,
}

impl IloldPrompt {
    fn label(&self) -> String {
        if self.scenario == "main" {
            self.contract.clone()
        } else {
            format!("{}/{}", self.contract, self.scenario)
        }
    }
}

impl Prompt for IloldPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        let label = self.label();
        if self.steps.is_empty() {
            Cow::Owned(format!("ilold[{}]", label))
        } else if self.steps.len() <= 3 {
            let path = self
                .steps
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" → ");
            Cow::Owned(format!("ilold[{} → {}]", label, path))
        } else {
            let skipped = self.steps.len() - 2;
            Cow::Owned(format!(
                "ilold[{} → {} → ...{} more → {}]",
                label,
                self.steps[0],
                skipped,
                self.steps.last().unwrap()
            ))
        }
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(".. ")
    }
    fn render_prompt_history_search_indicator(
        &self,
        search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        match search.status {
            PromptHistorySearchStatus::Passing => Cow::Borrowed("(search) "),
            PromptHistorySearchStatus::Failing => Cow::Borrowed("(search failed) "),
        }
    }
}

struct IloldCompleter {
    functions: Vec<String>,
    contracts: Vec<String>,
    scenarios: Vec<String>,
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

        let needs_scenario = line_lower.starts_with("scenario switch ")
            || line_lower.starts_with("scenario delete ")
            || line_lower.starts_with("sc switch ")
            || line_lower.starts_with("sc delete ");

        if !needs_func && !needs_contract && !needs_scenario {
            return Vec::new();
        }

        let arg_start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let partial = &line[arg_start..pos];

        let source = if needs_scenario {
            &self.scenarios
        } else if needs_contract {
            &self.contracts
        } else {
            &self.functions
        };

        source
            .iter()
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
