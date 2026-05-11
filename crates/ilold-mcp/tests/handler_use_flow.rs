//! Integration tests for the agnostic-contract handler flow:
//! `ilold_use` switches the active program, other tools error when no contract
//! is active, and the active contract is forwarded verbatim to `/api/cmd`.

use std::sync::Arc;

use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use ilold_mcp::IloldClient;
use ilold_mcp::server::dispatch;
use serde_json::json;
use tokio::sync::Mutex;

fn project_map_two_programs(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/api/project/map");
        then.status(200).json_body(json!({
            "kind": "solana",
            "programs": [
                { "name": "hand", "program_id": "Hand1111", "instructions": [] },
                { "name": "lever", "program_id": "Lev11111", "instructions": [] }
            ]
        }));
    });
}

#[tokio::test]
async fn tool_call_without_active_contract_returns_error() {
    let server = MockServer::start_async().await;
    let client = Arc::new(IloldClient::new(server.base_url()));
    let state = Arc::new(Mutex::new(None));

    let res = dispatch(&client, &state, "ilold_funcs", None).await;
    assert_eq!(res.is_error, Some(true));
    let dumped = serde_json::to_string(&res).expect("serialize CallToolResult");
    assert!(
        dumped.contains("No active contract"),
        "expected guidance to call ilold_use, got: {dumped}"
    );
}

#[tokio::test]
async fn ilold_use_sets_active_contract() {
    let server = MockServer::start_async().await;
    project_map_two_programs(&server);
    let client = Arc::new(IloldClient::new(server.base_url()));
    let state = Arc::new(Mutex::new(None));

    let res = dispatch(
        &client,
        &state,
        "ilold_use",
        Some(&json!({ "program": "lever" })),
    )
    .await;
    assert_ne!(res.is_error, Some(true));
    let active = state.lock().await.clone();
    assert_eq!(active.as_deref(), Some("lever"));
}

#[tokio::test]
async fn ilold_use_rejects_unknown_program() {
    let server = MockServer::start_async().await;
    project_map_two_programs(&server);
    let client = Arc::new(IloldClient::new(server.base_url()));
    let state = Arc::new(Mutex::new(None));

    let res = dispatch(
        &client,
        &state,
        "ilold_use",
        Some(&json!({ "program": "ghost" })),
    )
    .await;
    assert_eq!(res.is_error, Some(true));
    assert!(state.lock().await.is_none());
}

#[tokio::test]
async fn tool_call_uses_active_contract_in_post_body() {
    let server = MockServer::start_async().await;
    project_map_two_programs(&server);
    let cmd_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/cmd")
            .json_body_partial(r#"{ "contract": "hand" }"#);
        then.status(200).json_body(json!({
            "InstructionList": { "items": [] }
        }));
    });

    let client = Arc::new(IloldClient::new(server.base_url()));
    let state = Arc::new(Mutex::new(None));

    let set = dispatch(
        &client,
        &state,
        "ilold_use",
        Some(&json!({ "program": "hand" })),
    )
    .await;
    assert_ne!(set.is_error, Some(true));

    let call = dispatch(&client, &state, "ilold_funcs", None).await;
    assert_ne!(call.is_error, Some(true));
    cmd_mock.assert();
}

#[tokio::test]
async fn ilold_use_can_switch_between_programs() {
    let server = MockServer::start_async().await;
    project_map_two_programs(&server);
    let client = Arc::new(IloldClient::new(server.base_url()));
    let state = Arc::new(Mutex::new(Some("hand".to_string())));

    let switch = dispatch(
        &client,
        &state,
        "ilold_use",
        Some(&json!({ "program": "lever" })),
    )
    .await;
    assert_ne!(switch.is_error, Some(true));
    assert_eq!(state.lock().await.as_deref(), Some("lever"));
}
