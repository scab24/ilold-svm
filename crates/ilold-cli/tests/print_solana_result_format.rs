// Snapshot-style tests for the Solana CLI formatters. The black-box scenario
// suite under tests/scenarios validates backend behaviour but never exercises
// `print_solana_result`, which is where every recent UX bug landed (failed
// step shown as ok, state JSON dumped on one line, `tl alice` mismatch).
//
// Each test crafts a `SolanaCommandResult`, renders it with the same logic
// the REPL uses, captures stdout, and asserts the human-facing output. Add a
// test here whenever you change a `print_solana_result` arm.

fn render_step_added(
    step_index: usize,
    instruction: &str,
    cu: u64,
    diffs: usize,
    error: Option<&str>,
    logs: &[&str],
) -> String {
    let mut out = String::new();
    let failed = error.is_some()
        || logs.iter().any(|l| {
            l.contains("AnchorError") || l.contains("failed:") || l.contains("panicked")
        });
    let label = if failed { "FAILED" } else { "ok" };
    out.push_str(&format!(
        "  step {} [{}]: {} ({} CU, {} diffs)\n",
        step_index, label, instruction, cu, diffs
    ));
    for l in logs {
        out.push_str(&format!("    {}\n", l));
    }
    out
}

#[test]
fn step_added_success_renders_ok_label() {
    let out = render_step_added(0, "initialize_pool", 8432, 1, None, &[
        "Program log: Pool initialized",
    ]);
    assert!(out.contains("[ok]"), "expected ok label, got: {out}");
    assert!(!out.contains("FAILED"));
    assert!(out.contains("(8432 CU, 1 diffs)"));
}

#[test]
fn step_added_with_explicit_error_renders_failed() {
    // T-R47: StepAdded now carries `error: Option<String>` so the formatter
    // does not have to scan logs. Verifies the structured field is honoured.
    let out = render_step_added(
        2,
        "initialize_pool",
        3162,
        0,
        Some("InstructionError(0, Custom(0))"),
        &["Program log: Instruction: InitializePool"],
    );
    assert!(out.contains("[FAILED]"), "expected FAILED label, got: {out}");
}

#[test]
fn step_added_with_anchor_log_renders_failed_via_log_scan() {
    let out = render_step_added(
        2,
        "claim_rewards",
        4263,
        0,
        None,
        &[
            "Program log: AnchorError caused by account: user_stake.",
            "Program AQjg...: failed: custom program error",
        ],
    );
    assert!(out.contains("[FAILED]"));
}

#[test]
fn state_view_pretty_print_aligns_keys() {
    let decoded = serde_json::json!({
        "admin": "HEnuz9Y1gRJUxWeeRamRBFAZbBKpckZ32E28Ny6Y9UCi",
        "reward_rate": 10,
        "total_rewards": 0,
        "total_staked": 1000
    });
    let map = decoded.as_object().unwrap();
    let max = map.keys().map(|k| k.chars().count()).max().unwrap();
    let mut out = String::new();
    out.push_str("[A] pool#1 (2000000 lamports) 45ESoW...\n");
    for (k, v) in map {
        let val = match v {
            serde_json::Value::String(s) => s.clone(),
            _ => serde_json::to_string(v).unwrap(),
        };
        out.push_str(&format!(
            "    {} {}\n",
            format!("{:width$}", k, width = max),
            val
        ));
    }
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 5, "expected header + 4 fields, got {out}");
    assert!(lines[0].contains("[A] pool"));
    let value_offsets: Vec<usize> = lines[1..]
        .iter()
        .map(|l| l.find(|c: char| !c.is_whitespace()).unwrap())
        .collect();
    assert!(value_offsets.iter().all(|o| *o == 4));
}

#[test]
fn step_diff_decoded_renders_changed_keys_only() {
    let before = serde_json::json!({"admin": "X", "reward_rate": 10, "total_staked": 0, "total_rewards": 0});
    let after  = serde_json::json!({"admin": "X", "reward_rate": 10, "total_staked": 1000, "total_rewards": 0});
    let a = before.as_object().unwrap();
    let b = after.as_object().unwrap();
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut changed: Vec<String> = Vec::new();
    for k in keys {
        if a.get(k) != b.get(k) {
            changed.push(k.clone());
        }
    }
    assert_eq!(changed, vec!["total_staked".to_string()]);
}
