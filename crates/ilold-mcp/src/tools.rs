use std::sync::Arc;

use ilold_help::{HelpBlock, SOLANA_HELP_BLOCKS};
use rmcp::model::{JsonObject, Tool};
use serde_json::{Map, Value, json};

use crate::schema::schema_for_tool;

// `use` is a REPL-only meta command that switches the active program in the
// CLI prompt; it has no SolanaCommand variant and the MCP transport selects
// the program via the `--contract` flag instead, so the tool is excluded
// from the registry.
const EXCLUDED_ALIASES: &[&str] = &[
    "?", "help", "h", "quit", "q", "exit", "browser", "use",
];
const TOOL_NAME_PREFIX: &str = "ilold_";

pub fn build_tool_registry() -> Vec<Tool> {
    SOLANA_HELP_BLOCKS
        .iter()
        .filter(|b| !is_excluded(b))
        .map(|b| {
            let canonical = canonical_alias(b);
            let name = format!("{TOOL_NAME_PREFIX}{}", normalize_name(canonical));
            let description = format_description(b);
            let schema = value_to_json_object(schema_for_tool(&name));
            Tool::new(name, description, Arc::new(schema))
        })
        .collect()
}

pub fn is_excluded(block: &HelpBlock) -> bool {
    block
        .aliases
        .iter()
        .any(|alias| EXCLUDED_ALIASES.contains(alias))
}

pub fn canonical_alias(block: &HelpBlock) -> &'static str {
    // alias must have at least 4 chars to be canonical, so `seq` falls back to `sequence`
    block
        .aliases
        .iter()
        .copied()
        .find(|a| a.len() >= 4)
        .unwrap_or_else(|| block.aliases[0])
}

pub fn normalize_name(alias: &str) -> String {
    alias.replace('-', "_")
}

fn format_description(block: &HelpBlock) -> String {
    let mut out = String::new();
    out.push_str(block.purpose);
    if block.aliases.len() > 1 {
        out.push_str(&format!("\n\nAliases: {}", block.aliases.join(", ")));
    }
    if !block.returns.is_empty() {
        out.push_str(&format!("\n\nReturns: {}", block.returns));
    }
    out
}

fn value_to_json_object(v: Value) -> JsonObject {
    match v {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// Translate MCP tool `name` + `arguments` into the SolanaCommand JSON value
/// that the backend `/api/cmd` endpoint expects. Returns the JSON `command`
/// payload only — the `IloldClient` wraps it with the `contract` field.
pub fn build_command(name: &str, arguments: Option<&Value>) -> Result<Value, String> {
    let args = arguments.cloned().unwrap_or_else(|| json!({}));
    let args_obj = args.as_object().cloned().unwrap_or_default();
    match name {
        "ilold_call" => {
            let ix = require_str(&args_obj, "ix")?;
            let call_args = args_obj.get("args").cloned().unwrap_or_else(|| json!({}));
            let accounts = args_obj
                .get("accounts")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let signers = args_obj
                .get("signers")
                .cloned()
                .unwrap_or_else(|| json!([]));
            Ok(json!({
                "Call": {
                    "ix": ix,
                    "args": call_args,
                    "accounts": accounts,
                    "signers": signers,
                }
            }))
        }
        "ilold_back" => Ok(json!("Back")),
        "ilold_clear" => Ok(json!("Clear")),
        "ilold_funcs" | "ilold_functions" => Ok(json!("Funcs")),
        "ilold_funcs_all" => Ok(json!("Funcs")),
        "ilold_state" => Ok(json!("State")),
        "ilold_session" | "ilold_sequence" => Ok(json!("Session")),
        "ilold_info" => Ok(json!({ "Info": { "ix": require_str(&args_obj, "ix")? } })),
        "ilold_vars" => Ok(json!("Vars")),
        "ilold_users" => Ok(json!("Users")),
        "ilold_users_new" => {
            let name = require_str(&args_obj, "name")?;
            let mut obj = json!({ "name": name });
            if let Some(lamports) = args_obj.get("lamports") {
                obj["lamports"] = lamports.clone();
            }
            Ok(json!({ "UsersNew": obj }))
        }
        "ilold_airdrop" => {
            let user = require_str(&args_obj, "user")?;
            let lamports = args_obj
                .get("lamports")
                .ok_or_else(|| "missing required field: lamports".to_string())?
                .clone();
            Ok(json!({ "Airdrop": { "user": user, "lamports": lamports } }))
        }
        "ilold_time_warp" => {
            let delta = args_obj
                .get("delta_seconds")
                .ok_or_else(|| "missing required field: delta_seconds".to_string())?
                .clone();
            Ok(json!({ "TimeWarp": { "delta_seconds": delta } }))
        }
        "ilold_pda" => Ok(json!({ "Pda": { "instruction": require_str(&args_obj, "instruction")? } })),
        "ilold_inspect" => Ok(json!({ "Inspect": { "pubkey": require_str(&args_obj, "pubkey")? } })),
        "ilold_step" => {
            let index = args_obj
                .get("index")
                .ok_or_else(|| "missing required field: index".to_string())?
                .clone();
            Ok(json!({ "Step": { "index": index } }))
        }
        "ilold_who" => Ok(json!({ "Who": { "account_type": require_str(&args_obj, "account_type")? } })),
        "ilold_timeline" => Ok(json!({ "Timeline": { "pubkey": require_str(&args_obj, "pubkey")? } })),
        "ilold_coupling" => Ok(json!("Coupling")),
        "ilold_coverage" => Ok(json!("Coverage")),
        "ilold_finding" => {
            let severity = require_str(&args_obj, "severity")?;
            let title = require_str(&args_obj, "title")?;
            let description = require_str(&args_obj, "description")?;
            let mut obj = json!({
                "severity": severity,
                "title": title,
                "description": description,
            });
            if let Some(rec) = args_obj.get("recommendation") {
                obj["recommendation"] = rec.clone();
            }
            Ok(json!({ "Finding": obj }))
        }
        "ilold_findings" => Ok(json!("Findings")),
        "ilold_note" => Ok(json!({ "Note": { "text": require_str(&args_obj, "text")? } })),
        "ilold_status" => Ok(json!({
            "Status": {
                "ix": require_str(&args_obj, "ix")?,
                "status": require_str(&args_obj, "status")?,
            }
        })),
        "ilold_export" => {
            if let Some(meta) = args_obj.get("metadata") {
                Ok(json!({ "Export": { "metadata": meta } }))
            } else {
                Ok(json!({ "Export": { "metadata": null } }))
            }
        }
        "ilold_scenario" => {
            let sub = args_obj
                .get("sub")
                .cloned()
                .ok_or_else(|| "missing required field: sub".to_string())?;
            Ok(json!({ "Scenario": { "sub": sub } }))
        }
        "ilold_save" => {
            let with_keypairs = args_obj
                .get("with_keypairs")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(json!({ "SaveSession": { "with_keypairs": with_keypairs } }))
        }
        "ilold_load" => Ok(json!({ "LoadSession": { "json": require_str(&args_obj, "json")? } })),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn require_str(obj: &Map<String, Value>, key: &str) -> Result<String, String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing or non-string field: {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ilold_solana_core::exploration::SolanaCommand;

    #[test]
    fn tool_registry_has_29_entries() {
        let tools = build_tool_registry();
        assert_eq!(tools.len(), 29);
    }

    #[test]
    fn tool_names_are_unique() {
        let tools = build_tool_registry();
        let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        names.sort();
        let dup = names.windows(2).find(|w| w[0] == w[1]);
        assert!(dup.is_none(), "duplicate tool name: {:?}", dup);
    }

    #[test]
    fn excluded_commands_not_in_registry() {
        let tools = build_tool_registry();
        let forbidden = [
            "ilold_?",
            "ilold_help",
            "ilold_h",
            "ilold_quit",
            "ilold_q",
            "ilold_exit",
            "ilold_browser",
            "ilold_use",
        ];
        for f in forbidden {
            assert!(
                !tools.iter().any(|t| t.name == f),
                "registry should not contain {f}"
            );
        }
    }

    #[test]
    fn canonical_alias_prefers_long_form() {
        let block = HelpBlock {
            title: "c | call",
            aliases: &["c", "call"],
            purpose: "",
            syntax: &[],
            flags: &[],
            examples: &[],
            returns: "",
            see_also: &[],
        };
        assert_eq!(canonical_alias(&block), "call");
    }

    #[test]
    fn canonical_alias_falls_back_to_first_when_all_short() {
        let block = HelpBlock {
            title: "? | h",
            aliases: &["?", "h"],
            purpose: "",
            syntax: &[],
            flags: &[],
            examples: &[],
            returns: "",
            see_also: &[],
        };
        assert_eq!(canonical_alias(&block), "?");
    }

    #[test]
    fn normalize_name_replaces_dash() {
        assert_eq!(normalize_name("funcs-all"), "funcs_all");
        assert_eq!(normalize_name("time-warp"), "time_warp");
    }

    #[test]
    fn normalize_name_idempotent() {
        let once = normalize_name("time-warp");
        let twice = normalize_name(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn names_have_ilold_prefix() {
        let tools = build_tool_registry();
        for t in &tools {
            assert!(
                t.name.starts_with(TOOL_NAME_PREFIX),
                "tool name missing prefix: {}",
                t.name
            );
        }
    }

    #[test]
    fn description_includes_purpose_and_returns() {
        let tools = build_tool_registry();
        let call = tools.iter().find(|t| t.name == "ilold_call").expect("call");
        let desc = call.description.as_deref().unwrap_or("");
        assert!(desc.contains("Anchor instruction"));
        assert!(desc.contains("Returns:"));
    }

    #[test]
    fn build_command_call_translates_minimal() {
        let v = build_command("ilold_call", Some(&json!({"ix": "stake"}))).unwrap();
        let cmd: SolanaCommand = serde_json::from_value(v).expect("deserialize");
        match cmd {
            SolanaCommand::Call { ix, args, accounts, signers } => {
                assert_eq!(ix, "stake");
                assert!(args.as_object().is_some_and(|o| o.is_empty()));
                assert!(accounts.is_empty());
                assert!(signers.is_empty());
            }
            _ => panic!("expected Call variant"),
        }
    }

    #[test]
    fn build_command_funcs_unit_variant() {
        let v = build_command("ilold_funcs", None).unwrap();
        let cmd: SolanaCommand = serde_json::from_value(v).expect("deserialize");
        assert!(matches!(cmd, SolanaCommand::Funcs));
    }

    #[test]
    fn build_command_scenario_sub_passthrough() {
        let v = build_command(
            "ilold_scenario",
            Some(&json!({ "sub": { "New": { "name": "branch1" } } })),
        )
        .unwrap();
        let cmd: SolanaCommand = serde_json::from_value(v).expect("deserialize");
        assert!(matches!(cmd, SolanaCommand::Scenario { .. }));
    }

    #[test]
    fn build_command_users_new_includes_default_lamports() {
        // Lamports omitted on the wire — backend applies its own default.
        let v = build_command("ilold_users_new", Some(&json!({ "name": "alice" }))).unwrap();
        let cmd: SolanaCommand = serde_json::from_value(v).expect("deserialize");
        match cmd {
            SolanaCommand::UsersNew { name, lamports } => {
                assert_eq!(name, "alice");
                assert!(lamports > 0, "default lamports should be positive");
            }
            _ => panic!("expected UsersNew"),
        }
    }

    #[test]
    fn build_command_time_warp_passes_delta() {
        let v = build_command("ilold_time_warp", Some(&json!({ "delta_seconds": 86400 }))).unwrap();
        let cmd: SolanaCommand = serde_json::from_value(v).expect("deserialize");
        match cmd {
            SolanaCommand::TimeWarp { delta_seconds } => assert_eq!(delta_seconds, 86400),
            _ => panic!("expected TimeWarp"),
        }
    }
}
