use serde_json::Value;

pub fn intent_for_tool(name: &str, args: Option<&Value>) -> String {
    let obj = args.and_then(|v| v.as_object());
    match name {
        "ilold_call" => narrate_call(args),
        "ilold_back" => "Reverting the last step".to_string(),
        "ilold_clear" => "Clearing the active scenario".to_string(),
        "ilold_state" => "Dumping mutated account state".to_string(),
        "ilold_session" => "Summarizing the active scenario".to_string(),
        "ilold_programs" => "Listing workspace programs".to_string(),
        "ilold_funcs" | "ilold_funcs_all" => "Listing instructions of the active program".to_string(),
        "ilold_info" => match obj.and_then(|o| o.get("ix")).and_then(|v| v.as_str()) {
            Some(ix) => format!("Inspecting instruction `{ix}`"),
            None => "Inspecting instruction".to_string(),
        },
        "ilold_vars" => "Listing account types and discriminators".to_string(),
        "ilold_users" => "Listing scenario keypairs".to_string(),
        "ilold_users_new" => narrate_users_new(args),
        "ilold_airdrop" => narrate_airdrop(args),
        "ilold_time_warp" => narrate_time_warp(args),
        "ilold_pda" => match obj.and_then(|o| o.get("instruction")).and_then(|v| v.as_str()) {
            Some(ix) => format!("Listing PDAs of instruction `{ix}`"),
            None => "Listing PDAs".to_string(),
        },
        "ilold_inspect" => match obj.and_then(|o| o.get("pubkey")).and_then(|v| v.as_str()) {
            Some(p) => format!("Decoding account `{p}`"),
            None => "Decoding account".to_string(),
        },
        "ilold_step" => match obj.and_then(|o| o.get("index")) {
            Some(idx) => format!("Re-inspecting step {idx}"),
            None => "Re-inspecting step".to_string(),
        },
        "ilold_who" => match obj.and_then(|o| o.get("account_type")).and_then(|v| v.as_str()) {
            Some(t) => format!("Looking up who touches account type `{t}`"),
            None => "Looking up cross-references".to_string(),
        },
        "ilold_timeline" => match obj.and_then(|o| o.get("pubkey")).and_then(|v| v.as_str()) {
            Some(p) => format!("Tracing timeline of account `{p}`"),
            None => "Tracing account timeline".to_string(),
        },
        "ilold_coupling" => "Computing instruction coupling".to_string(),
        "ilold_coverage" => "Computing runtime coverage".to_string(),
        "ilold_finding" => narrate_finding(args),
        "ilold_findings" => "Listing scenario findings".to_string(),
        "ilold_note" => "Adding a free-form note".to_string(),
        "ilold_status" => match (
            obj.and_then(|o| o.get("ix")).and_then(|v| v.as_str()),
            obj.and_then(|o| o.get("status")).and_then(|v| v.as_str()),
        ) {
            (Some(ix), Some(s)) => format!("Marking `{ix}` as `{s}`"),
            _ => "Updating instruction review status".to_string(),
        },
        "ilold_export" => "Generating the deliverable export".to_string(),
        "ilold_scenario" => narrate_scenario(args),
        "ilold_save" => "Saving the scenario snapshot".to_string(),
        "ilold_load" => "Restoring a scenario from JSON".to_string(),
        other => format!("Calling `{other}`"),
    }
}

fn narrate_call(args: Option<&Value>) -> String {
    let Some(obj) = args.and_then(|v| v.as_object()) else {
        return "Calling instruction".to_string();
    };
    let ix = obj.get("ix").and_then(|v| v.as_str()).unwrap_or("?");
    let mut bits: Vec<String> = Vec::new();
    if let Some(a) = obj.get("args").and_then(|v| v.as_object()) {
        for (k, v) in a.iter().take(2) {
            bits.push(format!("{k}={}", short_value(v)));
        }
    }
    if let Some(acc) = obj.get("accounts").and_then(|v| v.as_object()) {
        for (k, v) in acc.iter().take(2) {
            bits.push(format!("{k}={}", short_value(v)));
        }
    }
    if bits.is_empty() {
        format!("Calling `{ix}`")
    } else {
        format!("Calling `{ix}` with {}", bits.join(", "))
    }
}

fn narrate_users_new(args: Option<&Value>) -> String {
    let name = args
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("name"))
        .and_then(|v| v.as_str());
    match name {
        Some(n) => format!("Creating a new keypair `{n}`"),
        None => "Creating a new keypair".to_string(),
    }
}

fn narrate_airdrop(args: Option<&Value>) -> String {
    let obj = args.and_then(|v| v.as_object());
    let user = obj.and_then(|o| o.get("user")).and_then(|v| v.as_str());
    let lamports = obj.and_then(|o| o.get("lamports"));
    match (user, lamports) {
        (Some(u), Some(l)) => format!("Airdropping {l} lamports to `{u}`"),
        (Some(u), None) => format!("Airdropping to `{u}`"),
        _ => "Airdropping lamports".to_string(),
    }
}

fn narrate_time_warp(args: Option<&Value>) -> String {
    match args
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("delta_seconds"))
    {
        Some(d) => format!("Advancing the clock by {d}s"),
        None => "Advancing the clock".to_string(),
    }
}

fn narrate_finding(args: Option<&Value>) -> String {
    let obj = args.and_then(|v| v.as_object());
    let sev = obj.and_then(|o| o.get("severity")).and_then(|v| v.as_str());
    let title = obj.and_then(|o| o.get("title")).and_then(|v| v.as_str());
    match (sev, title) {
        (Some(s), Some(t)) => format!("Recording {s} finding: `{t}`"),
        (None, Some(t)) => format!("Recording finding: `{t}`"),
        _ => "Recording finding".to_string(),
    }
}

fn narrate_scenario(args: Option<&Value>) -> String {
    let sub = args
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("sub"));
    let Some(sub_obj) = sub.and_then(|v| v.as_object()) else {
        return "Running scenario sub-command".to_string();
    };
    if let Some((variant, payload)) = sub_obj.iter().next() {
        let name = payload
            .as_object()
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str());
        return match name {
            Some(n) => format!("Scenario `{variant}` on `{n}`"),
            None => format!("Scenario `{variant}`"),
        };
    }
    "Running scenario sub-command".to_string()
}

fn short_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        other => {
            let s = other.to_string();
            if s.len() > 32 {
                format!("{}…", &s[..32])
            } else {
                s
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn narrate_call_includes_ix_and_first_args() {
        let args = json!({ "ix": "stake", "args": { "amount": 1000 } });
        let out = intent_for_tool("ilold_call", Some(&args));
        assert!(out.contains("stake"));
        assert!(out.contains("amount=1000"));
    }

    #[test]
    fn narrate_call_truncates_at_two_pairs() {
        let args = json!({
            "ix": "stake",
            "args": { "a": 1, "b": 2, "c": 3, "d": 4, "e": 5 }
        });
        let out = intent_for_tool("ilold_call", Some(&args));
        assert!(out.contains("a=1"));
        assert!(out.contains("b=2"));
        assert!(!out.contains("c=3"), "should truncate after two pairs: {out}");
    }

    #[test]
    fn narrate_call_combines_args_and_accounts() {
        let args = json!({
            "ix": "stake",
            "args": { "amount": 1000 },
            "accounts": { "user": "alice" },
        });
        let out = intent_for_tool("ilold_call", Some(&args));
        assert!(out.contains("amount=1000"));
        assert!(out.contains("user=alice"));
    }

    #[test]
    fn narrate_users_new_includes_name() {
        let args = json!({ "name": "alice" });
        let out = intent_for_tool("ilold_users_new", Some(&args));
        assert!(out.contains("alice"));
        assert!(out.to_lowercase().contains("keypair"));
    }

    #[test]
    fn narrate_who_includes_account_type() {
        let args = json!({ "account_type": "Pool" });
        let out = intent_for_tool("ilold_who", Some(&args));
        assert!(out.contains("Pool"));
    }

    #[test]
    fn narrate_unknown_tool_falls_back_to_generic() {
        let out = intent_for_tool("ilold_made_up", None);
        assert_eq!(out, "Calling `ilold_made_up`");
    }

    #[test]
    fn narrate_coverage_is_short() {
        let out = intent_for_tool("ilold_coverage", None);
        assert_eq!(out, "Computing runtime coverage");
    }

    #[test]
    fn narrate_call_with_empty_args_returns_minimal() {
        let args = json!({ "ix": "stake" });
        let out = intent_for_tool("ilold_call", Some(&args));
        assert_eq!(out, "Calling `stake`");
    }

    #[test]
    fn narrate_scenario_includes_sub_variant() {
        let args = json!({ "sub": { "New": { "name": "branch1" } } });
        let out = intent_for_tool("ilold_scenario", Some(&args));
        assert!(out.contains("New"));
        assert!(out.contains("branch1"));
    }
}
