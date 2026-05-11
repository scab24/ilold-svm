use schemars::Schema;
use serde_json::{Map, Value, json};

/// JSON Schema for the input arguments of a tool. The MCP spec requires
/// `inputSchema` to be a JSON object describing the `arguments` payload.
/// We hand-roll the schemas per tool (rather than slicing the big
/// SolanaCommand schema) because some variants need MCP-specific shapes:
/// for tools without arguments we return the canonical empty-object schema,
/// for tools with arguments we lift the variant fields to the top level so
/// the LLM sees `{ ix, args, accounts }` instead of `{ Call: { ix, ... } }`.
pub fn schema_for_tool(name: &str) -> Value {
    match name {
        "ilold_call" => schema_for_call(),
        "ilold_use" => string_only(
            "program",
            "Program name to make active (must match an entry returned by ilold_programs)",
        ),
        "ilold_info" => string_only("ix", "Instruction name to inspect"),
        "ilold_pda" => string_only("instruction", "Instruction whose PDAs to list"),
        "ilold_inspect" => string_only("pubkey", "Account pubkey (or named keypair)"),
        "ilold_who" => string_only(
            "account_type",
            "Query target: account type, instruction or struct field",
        ),
        "ilold_timeline" => string_only("pubkey", "Account pubkey (or named keypair)"),
        "ilold_users_new" => schema_users_new(),
        "ilold_airdrop" => schema_airdrop(),
        "ilold_time_warp" => schema_time_warp(),
        "ilold_finding" => schema_finding(),
        "ilold_note" => string_only("text", "Free-form annotation body"),
        "ilold_status" => schema_status(),
        "ilold_step" => schema_step(),
        "ilold_save" => schema_save(),
        "ilold_load" => string_only("json", "Saved scenario JSON to restore"),
        "ilold_scenario" => schema_scenario(),
        "ilold_export" => schema_export(),
        // Tools without arguments (Funcs, Back, Clear, State, Session,
        // Users, Vars, Findings, Coupling, Coverage, FuncsAll, Programs,
        // Sequence). Programs is a synthesised name; the registry uses
        // `ilold_programs` for the workspace listing handler.
        _ => empty_object_schema(),
    }
}

fn empty_object_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

fn string_only(field: &str, description: &str) -> Value {
    let mut props = Map::new();
    props.insert(
        field.to_string(),
        json!({ "type": "string", "description": description }),
    );
    json!({
        "type": "object",
        "required": [field],
        "properties": props,
        "additionalProperties": false
    })
}

fn schema_for_call() -> Value {
    json!({
        "type": "object",
        "required": ["ix"],
        "properties": {
            "ix": {
                "type": "string",
                "description": "Instruction name, e.g. stake or initialize_pool. Discover with ilold_funcs."
            },
            "args": {
                "type": "object",
                "description": "Anchor instruction args as a JSON object (e.g. {\"amount\": 1000}). Required keys depend on the instruction — call `ilold_info <ix>` first to discover them. Defaults to {}.",
                "default": {}
            },
            "accounts": {
                "type": "object",
                "additionalProperties": { "type": "string" },
                "description": "Map of account-name (from the IDL) to user handle or pubkey (e.g. {\"pool\":\"pool\",\"user\":\"alice\"}).",
                "default": {}
            },
            "signers": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Override the default signer set. Empty list means use the IDL defaults.",
                "default": []
            }
        },
        "additionalProperties": false
    })
}

fn schema_users_new() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": { "type": "string", "description": "Handle for the new keypair (e.g. alice)" },
            "lamports": {
                "type": "integer",
                "minimum": 0,
                "description": "Initial airdrop in lamports (defaults to 10_000_000_000 = 10 SOL)"
            }
        },
        "additionalProperties": false
    })
}

fn schema_airdrop() -> Value {
    json!({
        "type": "object",
        "required": ["user", "lamports"],
        "properties": {
            "user": { "type": "string", "description": "Existing user handle" },
            "lamports": {
                "type": "integer",
                "minimum": 0,
                "description": "Extra lamports to add"
            }
        },
        "additionalProperties": false
    })
}

fn schema_time_warp() -> Value {
    json!({
        "type": "object",
        "required": ["delta_seconds"],
        "properties": {
            "delta_seconds": {
                "type": "integer",
                "description": "Seconds to advance (positive) or rewind (negative)"
            }
        },
        "additionalProperties": false
    })
}

fn schema_finding() -> Value {
    json!({
        "type": "object",
        "required": ["severity", "title", "description"],
        "properties": {
            "severity": {
                "type": "string",
                "enum": ["Critical", "High", "Medium", "Low", "Informational"]
            },
            "title": { "type": "string" },
            "description": { "type": "string" },
            "recommendation": {
                "type": "string",
                "description": "Optional remediation suggestion"
            }
        },
        "additionalProperties": false
    })
}

fn schema_status() -> Value {
    json!({
        "type": "object",
        "required": ["ix", "status"],
        "properties": {
            "ix": { "type": "string", "description": "Instruction name" },
            "status": {
                "type": "string",
                "enum": [
                    "NotReviewed",
                    "InProgress",
                    "Reviewed",
                    "Suspicious",
                    "Vulnerable",
                    "Clean"
                ]
            }
        },
        "additionalProperties": false
    })
}

fn schema_step() -> Value {
    json!({
        "type": "object",
        "required": ["index"],
        "properties": {
            "index": {
                "type": "integer",
                "minimum": 0,
                "description": "Zero-based step index to re-inspect"
            }
        },
        "additionalProperties": false
    })
}

fn schema_save() -> Value {
    json!({
        "type": "object",
        "properties": {
            "with_keypairs": {
                "type": "boolean",
                "description": "When true the saved JSON embeds the per-scenario keypairs (do NOT commit the file).",
                "default": false
            }
        },
        "additionalProperties": false
    })
}

fn schema_scenario() -> Value {
    json!({
        "type": "object",
        "required": ["sub"],
        "properties": {
            "sub": {
                "description": "Sub-command. Use the variant matching the desired action.",
                "oneOf": [
                    { "type": "string", "enum": ["List"], "description": "List all scenarios" },
                    {
                        "type": "object",
                        "required": ["New"],
                        "properties": { "New": {
                            "type": "object",
                            "required": ["name"],
                            "properties": { "name": { "type": "string" } },
                            "additionalProperties": false
                        } },
                        "additionalProperties": false
                    },
                    {
                        "type": "object",
                        "required": ["Switch"],
                        "properties": { "Switch": {
                            "type": "object",
                            "required": ["name"],
                            "properties": { "name": { "type": "string" } },
                            "additionalProperties": false
                        } },
                        "additionalProperties": false
                    },
                    {
                        "type": "object",
                        "required": ["Fork"],
                        "properties": { "Fork": {
                            "type": "object",
                            "required": ["name"],
                            "properties": {
                                "name": { "type": "string" },
                                "at_step": { "type": "integer", "minimum": 0 }
                            },
                            "additionalProperties": false
                        } },
                        "additionalProperties": false
                    },
                    {
                        "type": "object",
                        "required": ["Delete"],
                        "properties": { "Delete": {
                            "type": "object",
                            "required": ["name"],
                            "properties": { "name": { "type": "string" } },
                            "additionalProperties": false
                        } },
                        "additionalProperties": false
                    }
                ]
            }
        },
        "additionalProperties": false
    })
}

fn schema_export() -> Value {
    json!({
        "type": "object",
        "properties": {
            "metadata": {
                "type": "object",
                "description": "Optional report metadata pass-through.",
                "properties": {
                    "auditor": { "type": "string" },
                    "project_version": { "type": "string" },
                    "audit_date": { "type": "string", "description": "YYYY-MM-DD" }
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    })
}

/// Returns the schemars-generated schema for the full SolanaCommand enum.
/// Useful for diagnostics and for the schema_consistency cross-check test.
pub fn full_solana_command_schema() -> Schema {
    schemars::schema_for!(ilold_solana_core::exploration::SolanaCommand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_for_call_requires_ix() {
        let s = schema_for_tool("ilold_call");
        let req = s.get("required").and_then(|r| r.as_array()).unwrap();
        assert!(req.iter().any(|v| v.as_str() == Some("ix")));
    }

    #[test]
    fn schema_for_funcs_has_no_properties() {
        let s = schema_for_tool("ilold_funcs");
        let props = s.get("properties").and_then(|p| p.as_object()).unwrap();
        assert!(props.is_empty());
        assert_eq!(s.get("additionalProperties"), Some(&Value::Bool(false)));
    }

    #[test]
    fn schema_for_scenario_is_oneof() {
        let s = schema_for_tool("ilold_scenario");
        let sub = s
            .get("properties")
            .and_then(|p| p.get("sub"))
            .expect("sub property");
        let one_of = sub.get("oneOf").and_then(|o| o.as_array()).unwrap();
        assert_eq!(one_of.len(), 5);
    }

    #[test]
    fn schema_for_call_has_args_description() {
        let s = schema_for_tool("ilold_call");
        let args = s
            .get("properties")
            .and_then(|p| p.get("args"))
            .expect("args property");
        let desc = args.get("description").and_then(|d| d.as_str()).unwrap();
        assert!(desc.contains("ilold_info"));
    }

    #[test]
    fn full_solana_command_schema_is_object() {
        let s = full_solana_command_schema();
        let v = serde_json::to_value(&s).unwrap();
        assert!(v.is_object());
    }
}
