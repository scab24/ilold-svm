//! Integration tests verifying existing `/api/session/*` endpoints target the
//! ACTIVE scenario after the ScenarioStore refactor.
//!
//! These are regression tests: the behavior with the single (auto-seeded)
//! "main" scenario must be unchanged. The spec S12 case "switching active
//! scenario flips endpoint output" lands in Phase S2 when the switch command
//! exists.

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

async fn start_with_staking() -> (reqwest::Client, u16) {
    let paths = vec![fixture("staking.sol")];
    let (_state, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();
    let client = reqwest::Client::new();

    // Add a step to the auto-seeded active "main" scenario.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({
            "contract": "Staking",
            "command": { "Call": { "func": "deposit" } }
        }))
        .send()
        .await
        .expect("POST /api/cmd failed");
    assert!(
        res.status().is_success(),
        "seed POST /api/cmd returned {}",
        res.status()
    );

    (client, port)
}

#[tokio::test]
async fn existing_state_endpoint_targets_active_scenario() {
    let (client, port) = start_with_staking().await;

    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/state"))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "GET /api/session/state returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.is_array(),
        "response should be a JSON array of VariableSummary, got: {body}"
    );
    // `deposit` mutates `balances` and `totalStaked` on the active scenario.
    let arr = body.as_array().unwrap();
    assert!(
        !arr.is_empty(),
        "active scenario state should have at least one variable summary after `c deposit`"
    );
}

#[tokio::test]
async fn existing_sequence_endpoint_targets_active_scenario() {
    let (client, port) = start_with_staking().await;

    // `sequence` requires at least two steps in the active scenario.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({
            "contract": "Staking",
            "command": { "Call": { "func": "withdraw" } }
        }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "seed second step POST returned {}",
        res.status()
    );

    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/sequence"))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "GET /api/session/sequence returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.is_object(),
        "response should be a SequenceNarrative object, got: {body}"
    );
}

#[tokio::test]
async fn existing_timeline_endpoint_targets_active_scenario() {
    let (client, port) = start_with_staking().await;

    // `deposit` writes `balances[msg.sender]`.
    let res = client
        .get(format!(
            "http://127.0.0.1:{port}/api/session/timeline/balances"
        ))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "GET /api/session/timeline/balances returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.is_object(),
        "response should be a VariableTimeline object, got: {body}"
    );
}

#[tokio::test]
async fn existing_narrative_endpoint_targets_active_scenario() {
    let (client, port) = start_with_staking().await;

    let res = client
        .get(format!(
            "http://127.0.0.1:{port}/api/session/step/0/narrative"
        ))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "GET /api/session/step/0/narrative returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.is_object(),
        "response should be a FunctionNarrative object, got: {body}"
    );
}

#[tokio::test]
async fn existing_trace_endpoint_targets_active_scenario() {
    let (client, port) = start_with_staking().await;

    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/step/0/trace"))
        .send()
        .await
        .unwrap();

    assert!(
        res.status().is_success(),
        "GET /api/session/step/0/trace returned {}",
        res.status()
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.is_object(),
        "response should be a FlowTree object, got: {body}"
    );
}
