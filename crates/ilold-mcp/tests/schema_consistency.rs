//! Cross-check that every tool in the registry has:
//!   1. An inputSchema that validates against a sample `arguments` payload.
//!   2. A `build_command` mapping that produces a JSON value which
//!      round-trips into the correct `SolanaCommand` variant.
//!
//! The special-case `ilold_programs` skips the build_command leg because the
//! MCP server routes it to a REST GET, not the /api/cmd dispatcher.

use ilold_mcp::tools::build_tool_registry;
use ilold_solana_core::exploration::SolanaCommand;
use serde_json::{Value, json};

/// Sample `arguments` payload per tool — the minimal JSON that satisfies
/// the tool's inputSchema. Tools without arguments map to `None`.
fn sample_arguments(name: &str) -> Option<Value> {
    match name {
        "ilold_call" => Some(json!({ "ix": "stake" })),
        "ilold_use" => Some(json!({ "program": "staking" })),
        "ilold_info" => Some(json!({ "ix": "stake" })),
        "ilold_pda" => Some(json!({ "instruction": "stake" })),
        "ilold_inspect" => Some(json!({ "pubkey": "alice" })),
        "ilold_who" => Some(json!({ "account_type": "Pool" })),
        "ilold_timeline" => Some(json!({ "pubkey": "alice" })),
        "ilold_users_new" => Some(json!({ "name": "alice" })),
        "ilold_airdrop" => Some(json!({ "user": "alice", "lamports": 1_000_000_000u64 })),
        "ilold_time_warp" => Some(json!({ "delta_seconds": 86400 })),
        "ilold_finding" => Some(json!({
            "severity": "High",
            "title": "reentrancy",
            "description": "found a reentrant path",
        })),
        "ilold_note" => Some(json!({ "text": "suspicious admin path" })),
        "ilold_status" => Some(json!({ "ix": "stake", "status": "Reviewed" })),
        "ilold_step" => Some(json!({ "index": 0 })),
        "ilold_load" => Some(json!({ "json": "{}" })),
        "ilold_scenario" => Some(json!({ "sub": { "New": { "name": "branch1" } } })),
        "ilold_save" => Some(json!({})),
        "ilold_export" => Some(json!({})),
        _ => None,
    }
}

/// `ilold_programs` and `ilold_use` are special-cases in the dispatcher:
/// `ilold_programs` calls REST GET /api/project/map and `ilold_use` updates
/// the handler's active-contract state. Neither has a SolanaCommand variant,
/// so we only validate the schema, not the build_command path.
fn skips_build_command(name: &str) -> bool {
    matches!(name, "ilold_programs" | "ilold_use")
}

#[test]
fn every_tool_input_schema_validates_sample() {
    let tools = build_tool_registry();
    for t in &tools {
        let schema_value: Value =
            serde_json::to_value(&*t.input_schema).expect("schema serializes");
        let validator = jsonschema::validator_for(&schema_value)
            .unwrap_or_else(|e| panic!("{}: schema compile failed: {e}", t.name));

        let args = sample_arguments(&t.name).unwrap_or_else(|| json!({}));
        let errors: Vec<String> = validator
            .iter_errors(&args)
            .map(|e| e.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "{}: sample {} failed schema: {:?}",
            t.name,
            args,
            errors
        );
    }
}

#[test]
fn every_tool_build_command_roundtrips_to_solana_command() {
    let tools = build_tool_registry();
    for t in &tools {
        if skips_build_command(&t.name) {
            continue;
        }
        let args = sample_arguments(&t.name);
        let cmd_json = ilold_mcp::tools::build_command(&t.name, args.as_ref())
            .unwrap_or_else(|e| panic!("{}: build_command failed: {e}", t.name));
        let _: SolanaCommand = serde_json::from_value(cmd_json.clone()).unwrap_or_else(|e| {
            panic!(
                "{}: SolanaCommand deserialize failed: {e} (payload: {})",
                t.name, cmd_json
            )
        });
    }
}

#[test]
fn users_new_schema_requires_name() {
    let tools = build_tool_registry();
    let t = tools
        .iter()
        .find(|t| t.name == "ilold_users_new")
        .expect("ilold_users_new present");
    let schema: Value = serde_json::to_value(&*t.input_schema).unwrap();
    let req = schema
        .get("required")
        .and_then(|r| r.as_array())
        .expect("required field");
    assert!(req.iter().any(|v| v.as_str() == Some("name")));
}

#[test]
fn sequence_tool_absent_from_registry() {
    let tools = build_tool_registry();
    assert!(
        !tools.iter().any(|t| t.name == "ilold_sequence"),
        "ilold_sequence must be excluded: /api/session/sequence is Solidity-only"
    );
}
