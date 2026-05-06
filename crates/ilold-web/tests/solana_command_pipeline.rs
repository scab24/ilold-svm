use std::path::PathBuf;

use ilold_solana_core::ingest::detect;

fn cpi_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/solana/cpi")
}

async fn start_solana() -> (reqwest::Client, u16) {
    let detected = detect(&cpi_fixture()).expect("detect cpi fixture");
    let (_state, port) = ilold_web::start_solana_server(detected, 0)
        .await
        .expect("start solana server");
    (reqwest::Client::new(), port)
}

async fn cmd(
    client: &reqwest::Client,
    port: u16,
    program: &str,
    command: serde_json::Value,
) -> serde_json::Value {
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({ "contract": program, "command": command }))
        .send()
        .await
        .expect("POST /api/cmd failed");
    assert!(
        res.status().is_success(),
        "POST /api/cmd returned {}",
        res.status()
    );
    res.json().await.expect("response was not JSON")
}

#[tokio::test]
async fn solana_funcs_lists_lever_instructions() {
    let (client, port) = start_solana().await;
    let result = cmd(&client, port, "lever", serde_json::json!("Funcs")).await;
    let items = result
        .get("InstructionList")
        .and_then(|v| v.get("items"))
        .and_then(|v| v.as_array())
        .expect("InstructionList.items");
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn solana_users_new_then_list() {
    let (client, port) = start_solana().await;
    let _ = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"UsersNew": {"name": "admin", "lamports": 5_000_000_000u64}}),
    )
    .await;
    let listed = cmd(&client, port, "lever", serde_json::json!("Users")).await;
    let users = listed
        .get("UserList")
        .and_then(|v| v.get("users"))
        .and_then(|v| v.as_array())
        .expect("UserList.users");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].get("name").and_then(|v| v.as_str()), Some("admin"));
}

#[tokio::test]
async fn solana_time_warp_returns_new_clock() {
    let (client, port) = start_solana().await;
    let result = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"TimeWarp": {"delta_seconds": 86400}}),
    )
    .await;
    let warped = result.get("TimeWarped").expect("TimeWarped variant");
    assert!(warped.get("unix_timestamp").is_some());
    assert!(warped.get("slot").is_some());
}

#[tokio::test]
async fn solana_scenario_fork_clones_vm_and_users() {
    let (client, port) = start_solana().await;

    let _ = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"UsersNew": {"name": "admin", "lamports": 1_000_000_000u64}}),
    )
    .await;

    let forked = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"Scenario": {"sub": {"Fork": {"name": "branch1", "at_step": null}}}}),
    )
    .await;
    assert!(forked.get("ScenarioForked").is_some(), "got: {forked}");

    let _ = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"Scenario": {"sub": {"Switch": {"name": "branch1"}}}}),
    )
    .await;

    let listed = cmd(&client, port, "lever", serde_json::json!("Users")).await;
    let users = listed
        .get("UserList")
        .and_then(|v| v.get("users"))
        .and_then(|v| v.as_array())
        .expect("UserList.users");
    assert_eq!(users.len(), 1, "fork should have cloned the admin user");
    assert_eq!(users[0].get("name").and_then(|v| v.as_str()), Some("admin"));
}

#[tokio::test]
async fn solana_call_switch_power_via_http() {
    let (client, port) = start_solana().await;

    let _ = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"UsersNew": {"name": "admin", "lamports": 5_000_000_000u64}}),
    )
    .await;
    let _ = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"UsersNew": {"name": "power", "lamports": 5_000_000_000u64}}),
    )
    .await;

    let init = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({
            "Call": {
                "ix": "initialize",
                "args": {},
                "accounts": {
                    "power": "power",
                    "user": "admin",
                    "system_program": "11111111111111111111111111111111"
                },
                "signers": ["admin", "power"]
            }
        }),
    )
    .await;
    assert!(init.get("StepAdded").is_some(), "got: {init}");

    let switch = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({
            "Call": {
                "ix": "switch_power",
                "args": {"name": "claude"},
                "accounts": {"power": "power"},
                "signers": []
            }
        }),
    )
    .await;
    let added = switch.get("StepAdded").expect("StepAdded for switch_power");
    let logs = added
        .get("logs_excerpt")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    assert!(
        logs.contains("pulling the power switch"),
        "expected lever log line, got: {logs}"
    );
}
