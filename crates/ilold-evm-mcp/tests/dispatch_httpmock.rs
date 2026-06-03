use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use ilold_evm_mcp::{dispatch, IloldClient};
use serde_json::json;

#[tokio::test]
async fn each_tool_routes_to_its_endpoint() {
    let server = MockServer::start();
    let overview = server.mock(|w, t| {
        w.method(GET).path("/api/project");
        t.status(200).json_body(json!({ "files": 1, "contracts": [] }));
    });
    let map = server.mock(|w, t| {
        w.method(GET).path("/api/project/map");
        t.status(200).json_body(json!({ "contracts": [], "relationships": [] }));
    });
    let project_dep = server.mock(|w, t| {
        w.method(GET).path("/api/project/depgraph");
        t.status(200).json_body(json!({ "nodes": [], "edges": [] }));
    });
    let contract_dep = server.mock(|w, t| {
        w.method(GET).path("/api/contract/Vault/depgraph");
        t.status(200).json_body(json!({ "nodes": [], "edges": [] }));
    });

    let client = IloldClient::new(server.base_url());

    assert!(!dispatch(&client, "ilold_project_overview", None).await.is_error.unwrap_or(false));
    overview.assert();

    assert!(!dispatch(&client, "ilold_project_map", None).await.is_error.unwrap_or(false));
    map.assert();

    assert!(!dispatch(&client, "ilold_dependency_graph", None).await.is_error.unwrap_or(false));
    project_dep.assert();

    let args = json!({ "contract": "Vault" });
    assert!(!dispatch(&client, "ilold_contract_dependencies", Some(&args)).await.is_error.unwrap_or(false));
    contract_dep.assert();
}

#[tokio::test]
async fn contract_dependencies_requires_contract() {
    let server = MockServer::start();
    let dep = server.mock(|w, t| {
        w.method(GET).path("/api/contract/Vault/depgraph");
        t.status(200).json_body(json!({}));
    });
    let client = IloldClient::new(server.base_url());

    let res = dispatch(&client, "ilold_contract_dependencies", None).await;
    assert_eq!(res.is_error, Some(true), "missing contract must error");
    assert_eq!(dep.hits(), 0, "no HTTP call when the required arg is absent");
}

#[tokio::test]
async fn unknown_tool_errors() {
    let server = MockServer::start();
    let client = IloldClient::new(server.base_url());
    let res = dispatch(&client, "ilold_does_not_exist", None).await;
    assert_eq!(res.is_error, Some(true));
}

#[tokio::test]
async fn p2_get_tools_route_to_endpoints() {
    let server = MockServer::start();
    let detail = server.mock(|w, t| {
        w.method(GET).path("/api/contract/Vault");
        t.status(200).json_body(json!({}));
    });
    let narrative = server.mock(|w, t| {
        w.method(GET).path("/api/session/function/Vault/deposit");
        t.status(200).json_body(json!({}));
    });
    let trace = server.mock(|w, t| {
        w.method(GET)
            .path("/api/session/trace/Vault/deposit")
            .query_param("depth", "3")
            .query_param("reverts", "true");
        t.status(200).json_body(json!({}));
    });
    let client = IloldClient::new(server.base_url());

    dispatch(&client, "ilold_contract_detail", Some(&json!({"contract":"Vault"}))).await;
    detail.assert();
    dispatch(&client, "ilold_function_analysis", Some(&json!({"contract":"Vault","function":"deposit"}))).await;
    narrative.assert();
    dispatch(&client, "ilold_trace", Some(&json!({"contract":"Vault","function":"deposit","depth":3,"reverts":true}))).await;
    trace.assert();
}

#[tokio::test]
async fn p2_post_tools_send_exact_command() {
    let server = MockServer::start();
    let entry = server.mock(|w, t| {
        w.method(POST)
            .path("/api/cmd")
            .json_body(json!({ "contract": "Vault", "command": "Functions" }));
        t.status(200).json_body(json!({ "FunctionList": { "functions": [] } }));
    });
    let who = server.mock(|w, t| {
        w.method(POST)
            .path("/api/cmd")
            .json_body(json!({ "contract": "Vault", "command": { "Who": { "variable": "total" } } }));
        t.status(200).json_body(json!({ "VariableInfo": {} }));
    });
    let client = IloldClient::new(server.base_url());

    dispatch(&client, "ilold_entry_points", Some(&json!({"contract":"Vault"}))).await;
    entry.assert();
    dispatch(&client, "ilold_who_touches", Some(&json!({"contract":"Vault","variable":"total"}))).await;
    who.assert();
}

#[tokio::test]
async fn p2_missing_required_args_error() {
    let server = MockServer::start();
    let client = IloldClient::new(server.base_url());
    let no_func = dispatch(&client, "ilold_function_analysis", Some(&json!({"contract":"Vault"}))).await;
    assert_eq!(no_func.is_error, Some(true), "function_analysis needs function");
    let no_var = dispatch(&client, "ilold_who_touches", Some(&json!({"contract":"Vault"}))).await;
    assert_eq!(no_var.is_error, Some(true), "who_touches needs variable");
}
