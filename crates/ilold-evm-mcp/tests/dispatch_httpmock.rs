use httpmock::Method::GET;
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
