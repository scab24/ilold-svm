use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::{json, Map, Value};

pub fn build_tool_registry() -> Vec<Tool> {
    vec![
        tool(
            "ilold_project_overview",
            "List every contract in the project with its kind, source folder, function and state-variable counts, and inheritance. Call this first to orient.",
            empty_schema(),
        ),
        tool(
            "ilold_project_map",
            "Full project topology: every contract with its functions and state variables, plus the resolved cross-contract call relationships.",
            empty_schema(),
        ),
        tool(
            "ilold_dependency_graph",
            "Contract dependency graph: inherits/calls/holds edges with topological reading-order layers. Shows which contracts are foundational and which depend on them.",
            empty_schema(),
        ),
        tool(
            "ilold_contract_dependencies",
            "Dependencies of one contract and its blast radius: the contracts it depends on, and the contracts that depend on it.",
            contract_schema(),
        ),
    ]
}

fn tool(name: &str, description: &str, schema: Value) -> Tool {
    Tool::new(name.to_string(), description.to_string(), Arc::new(to_object(schema)))
}

fn to_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn empty_schema() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn contract_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "contract": { "type": "string", "description": "Contract name" }
        },
        "required": ["contract"],
        "additionalProperties": false
    })
}
