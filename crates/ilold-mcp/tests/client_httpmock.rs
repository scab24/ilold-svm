//! Unit tests for `IloldClient` against a real (local) HTTP mock server.
//!
//! Covers SDD-05 § T-R55b.2.10 sub-tasks:
//!   - body serialization for `/api/cmd`
//!   - health-check rejecting non-Solana projects
//!   - `contract` field forwarded verbatim

use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use ilold_mcp::IloldClient;
use ilold_solana_core::exploration::SolanaCommandResult;
use serde_json::json;

#[tokio::test]
async fn client_send_command_serializes_body_correctly() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/cmd")
            .json_body(json!({
                "contract": "staking",
                "command": {
                    "Call": {
                        "ix": "stake",
                        "args": { "amount": 1000 },
                        "accounts": {},
                        "signers": []
                    }
                }
            }));
        then.status(200).json_body(json!({
            "StepAdded": {
                "step_index": 0,
                "instruction": "stake",
                "logs_excerpt": [],
                "account_diffs_count": 0,
                "compute_units": 0,
                "error": null
            }
        }));
    });

    let client = IloldClient::new(server.base_url());
    let command = json!({
        "Call": {
            "ix": "stake",
            "args": { "amount": 1000 },
            "accounts": {},
            "signers": []
        }
    });
    let result = client
        .send_command("staking", command)
        .await
        .expect("send_command succeeds");
    match result {
        SolanaCommandResult::StepAdded { instruction, .. } => {
            assert_eq!(instruction, "stake");
        }
        other => panic!("expected StepAdded, got {other:?}"),
    }
    mock.assert();
}

#[tokio::test]
async fn client_health_check_rejects_solidity() {
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET).path("/api/project/map");
        then.status(200).json_body(json!({
            "kind": "solidity",
            "programs": []
        }));
    });

    let client = IloldClient::new(server.base_url());
    let err = client
        .health_check()
        .await
        .expect_err("solidity backend must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("not Solana") && msg.contains("solidity"),
        "expected NotSolana error, got: {msg}"
    );
}

#[tokio::test]
async fn client_passes_contract_field() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/cmd")
            .json_body_partial(r#"{ "contract": "lever" }"#);
        then.status(200).json_body(json!("NoteAdded"));
    });

    let client = IloldClient::new(server.base_url());
    let _ = client
        .send_command("lever", json!("Funcs"))
        .await
        .expect("send_command serializes contract");
    mock.assert();
}

#[tokio::test]
async fn client_health_check_accepts_solana() {
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET).path("/api/project/map");
        then.status(200).json_body(json!({
            "kind": "solana",
            "programs": [{ "name": "staking" }]
        }));
    });

    let client = IloldClient::new(server.base_url());
    client
        .health_check()
        .await
        .expect("solana backend should pass health check");
}
