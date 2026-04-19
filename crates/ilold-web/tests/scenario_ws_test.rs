use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

/// Boot a server on an ephemeral port with the staking fixture and open a
/// WS connection. Returns the HTTP client, port, and the connected WS
/// stream.
async fn start_with_ws() -> (
    reqwest::Client,
    u16,
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
) {
    let paths = vec![fixture("staking.sol")];
    let (_state, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("WS connection failed");
    (reqwest::Client::new(), port, ws)
}

async fn post_cmd(
    client: &reqwest::Client,
    port: u16,
    contract: &str,
    command: serde_json::Value,
) {
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({ "contract": contract, "command": command }))
        .send()
        .await
        .expect("POST /api/cmd failed");
    assert!(
        res.status().is_success(),
        "POST /api/cmd returned {}",
        res.status()
    );
}

async fn next_ws_json<S>(ws: &mut S) -> serde_json::Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let msg = timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("timed out waiting for WS message")
        .expect("WS stream ended")
        .expect("WS error");
    let text = msg.into_text().expect("not a text message");
    serde_json::from_str(&text).expect("WS payload was not JSON")
}

fn scenario_new(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "New": { "name": name } } } })
}

fn scenario_switch(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "Switch": { "name": name } } } })
}

fn scenario_delete(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "Delete": { "name": name } } } })
}

fn call(func: &str) -> serde_json::Value {
    serde_json::json!({ "Call": { "func": func } })
}

#[tokio::test]
async fn ws_event_session_add_node_carries_scenario_field() {
    let (client, port, mut ws) = start_with_ws().await;

    post_cmd(&client, port, "Staking", call("deposit")).await;

    let payload = next_ws_json(&mut ws).await;
    assert_eq!(payload["type"], "session_add_node");
    assert_eq!(payload["scenario"], "main");
    assert_eq!(payload["function"], "deposit");
    assert!(payload["step_index"].is_number());
}

#[tokio::test]
async fn ws_event_scenario_created_broadcasts_on_new() {
    let (client, port, mut ws) = start_with_ws().await;

    post_cmd(&client, port, "Staking", scenario_new("alt1")).await;

    let payload = next_ws_json(&mut ws).await;
    assert_eq!(
        payload["type"], "scenario_created",
        "expected scenario_created, got: {payload}"
    );
    assert_eq!(payload["name"], "alt1");
}

#[tokio::test]
async fn ws_event_scenario_switched_broadcasts_on_switch() {
    let (client, port, mut ws) = start_with_ws().await;

    post_cmd(&client, port, "Staking", scenario_new("alt1")).await;
    // consume the `scenario_created` event so the next read is the switch
    let _created = next_ws_json(&mut ws).await;

    post_cmd(&client, port, "Staking", scenario_switch("alt1")).await;

    let payload = next_ws_json(&mut ws).await;
    assert_eq!(
        payload["type"], "scenario_switched",
        "expected scenario_switched, got: {payload}"
    );
    assert_eq!(payload["from"], "main");
    assert_eq!(payload["to"], "alt1");
}

#[tokio::test]
async fn ws_event_scenario_deleted_broadcasts_on_delete() {
    let (client, port, mut ws) = start_with_ws().await;

    post_cmd(&client, port, "Staking", scenario_new("alt1")).await;
    let _created = next_ws_json(&mut ws).await;

    // Delete a non-active scenario (active is still "main")
    post_cmd(&client, port, "Staking", scenario_delete("alt1")).await;

    let payload = next_ws_json(&mut ws).await;
    assert_eq!(
        payload["type"], "scenario_deleted",
        "expected scenario_deleted, got: {payload}"
    );
    assert_eq!(payload["name"], "alt1");
}

#[tokio::test]
async fn canvas_patch_scenario_field_propagates_to_ws_handler() {
    // Explicit check that the scenario field on a data event matches the
    // active scenario at the moment the command was dispatched. We switch
    // to a new scenario first, then `c deposit`; the `session_add_node`
    // must carry `scenario: "alt1"`, not `"main"`.
    let (client, port, mut ws) = start_with_ws().await;

    post_cmd(&client, port, "Staking", scenario_new("alt1")).await;
    let _created = next_ws_json(&mut ws).await;

    post_cmd(&client, port, "Staking", scenario_switch("alt1")).await;
    let _switched = next_ws_json(&mut ws).await;

    post_cmd(&client, port, "Staking", call("deposit")).await;
    let payload = next_ws_json(&mut ws).await;

    assert_eq!(payload["type"], "session_add_node");
    assert_eq!(
        payload["scenario"], "alt1",
        "scenario field must track active scenario at call time, got: {payload}"
    );
    assert_eq!(payload["function"], "deposit");
}

#[tokio::test]
async fn ws_event_scenario_store_reloaded_broadcasts_after_load() {
    let (client, port, mut ws) = start_with_ws().await;

    // Build a tiny state and save it so we have a valid v2 JSON to load back.
    post_cmd(&client, port, "Staking", call("deposit")).await;
    let _added = next_ws_json(&mut ws).await; // session_add_node

    // SaveSession does not broadcast — POST and read the JSON inline.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({
            "contract": "Staking",
            "command": "SaveSession",
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let json = body["SessionSaved"]["json"].as_str().unwrap().to_string();

    // LoadSession must broadcast scenario_store_reloaded with the active name.
    post_cmd(
        &client,
        port,
        "Staking",
        serde_json::json!({ "LoadSession": { "json": json } }),
    )
    .await;
    let payload = next_ws_json(&mut ws).await;

    assert_eq!(payload["type"], "scenario_store_reloaded");
    assert_eq!(
        payload["active"], "main",
        "reload event must carry the post-load active scenario, got: {payload}"
    );
}

