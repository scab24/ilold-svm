use ilold_mcp::tools::build_tool_registry;

#[test]
fn registry_lists_29_solana_tools() {
    let tools = build_tool_registry();
    assert_eq!(tools.len(), 29);
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"ilold_call"));
    assert!(names.contains(&"ilold_funcs"));
    assert!(names.contains(&"ilold_coverage"));
    assert!(!names.contains(&"ilold_use"));
    assert!(!names.contains(&"ilold_help"));
    assert!(!names.contains(&"ilold_quit"));
}

#[test]
fn every_tool_has_valid_input_schema() {
    let tools = build_tool_registry();
    for t in &tools {
        let schema = serde_json::to_value(&*t.input_schema).expect("schema serializes");
        assert!(schema.is_object(), "{}: schema must be JSON object", t.name);
        assert_eq!(
            schema.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "{}: schema type must be 'object'",
            t.name
        );
    }
}

#[test]
fn destructive_tool_names_present() {
    // SDD-05 notes destructive tools rely on the MCP client's heuristic
    // "name contains clear|delete" to prompt the human. Make sure those
    // names actually exist verbatim.
    let tools = build_tool_registry();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"ilold_clear"));
    assert!(names.contains(&"ilold_scenario"));
}
