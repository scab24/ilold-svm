use std::sync::Arc;

use ilold_help::{HelpBlock, SOLANA_HELP_BLOCKS};
use rmcp::model::{JsonObject, Tool};
use serde_json::{Map, Value, json};

const EXCLUDED_ALIASES: &[&str] = &["?", "help", "h", "quit", "q", "exit", "browser"];
const TOOL_NAME_PREFIX: &str = "ilold_";

pub fn build_tool_registry() -> Vec<Tool> {
    SOLANA_HELP_BLOCKS
        .iter()
        .filter(|b| !is_excluded(b))
        .map(|b| {
            let canonical = canonical_alias(b);
            let name = format!("{TOOL_NAME_PREFIX}{}", normalize_name(canonical));
            let description = format_description(b);
            Tool::new(name, description, Arc::new(empty_object_schema()))
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
    block
        .aliases
        .iter()
        .copied()
        .find(|a| a.len() >= 3)
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

fn empty_object_schema() -> JsonObject {
    let v = json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    });
    match v {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_registry_not_empty() {
        let tools = build_tool_registry();
        assert!(
            tools.len() >= 20,
            "expected at least 20 tools, got {}",
            tools.len()
        );
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
    fn normalize_name_replaces_dash() {
        assert_eq!(normalize_name("funcs-all"), "funcs_all");
        assert_eq!(normalize_name("time-warp"), "time_warp");
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
}
