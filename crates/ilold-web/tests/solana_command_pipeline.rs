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

// Regression for T-R40: LiteSVM does not rotate latest_blockhash after
// send_transaction (verified in litesvm 0.6.1 src/lib.rs:1141-1188 — only
// expire_blockhash mutates the field). Without our explicit rotation, the
// 2nd Call shares the 1st's blockhash and silently fails with cu=0 / no
// state change. This test runs three consecutive switch_power calls and
// asserts that each one actually executed (cu > 0 and a fresh log line).
#[tokio::test]
async fn solana_consecutive_calls_actually_execute() {
    let (client, port) = start_solana().await;

    for n in ["admin", "power"] {
        let _ = cmd(
            &client,
            port,
            "lever",
            serde_json::json!({"UsersNew": {"name": n, "lamports": 5_000_000_000u64}}),
        )
        .await;
    }

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
    assert!(init.get("StepAdded").is_some(), "init: {init}");

    // Three consecutive switch_power calls — pre-T-R40 only the first ran.
    for label in ["alpha", "beta", "gamma"] {
        let res = cmd(
            &client,
            port,
            "lever",
            serde_json::json!({
                "Call": {
                    "ix": "switch_power",
                    "args": {"name": label},
                    "accounts": {"power": "power"},
                    "signers": []
                }
            }),
        )
        .await;
        let step = res.get("StepAdded").unwrap_or_else(|| panic!("StepAdded for {label}: {res}"));
        let cu = step
            .get("compute_units")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(cu > 0, "consecutive Call '{label}' executed with cu=0 — blockhash rotation regression");
    }
}

// Regression for T-R33: Back must rewind the VM to the pre-Call snapshot,
// not just remove the step from the timeline. Verified by re-issuing the
// same Call after Back and checking the state mutates as if it were the
// first time.
#[tokio::test]
async fn solana_back_rewinds_vm_state() {
    let (client, port) = start_solana().await;

    for n in ["admin", "power"] {
        let _ = cmd(
            &client,
            port,
            "lever",
            serde_json::json!({"UsersNew": {"name": n, "lamports": 5_000_000_000u64}}),
        )
        .await;
    }
    let _ = cmd(
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

    let _ = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({
            "Call": {
                "ix": "switch_power",
                "args": {"name": "first"},
                "accounts": {"power": "power"},
                "signers": []
            }
        }),
    )
    .await;
    let session_after_two = cmd(&client, port, "lever", serde_json::json!("Session")).await;
    assert_eq!(
        session_after_two
            .get("SessionView")
            .and_then(|v| v.get("steps"))
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(2)
    );

    let back = cmd(&client, port, "lever", serde_json::json!("Back")).await;
    assert!(back.get("StepRemoved").is_some(), "Back: {back}");

    // Re-issue the same switch_power; if Back didn't rewind the VM the second
    // time the program logs the "previous" state which would still reflect
    // "first". After T-R33 it should be back to whatever initialize left.
    let replay = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({
            "Call": {
                "ix": "switch_power",
                "args": {"name": "second"},
                "accounts": {"power": "power"},
                "signers": []
            }
        }),
    )
    .await;
    let logs = replay
        .get("StepAdded")
        .and_then(|v| v.get("logs_excerpt"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    assert!(
        logs.contains("pulling the power switch"),
        "switch_power should still run after Back+replay; got: {logs}"
    );
}

#[tokio::test]
async fn solana_info_returns_typed_ix_view() {
    let (client, port) = start_solana().await;
    let result = cmd(
        &client,
        port,
        "lever",
        serde_json::json!({"Info": {"ix": "switch_power"}}),
    )
    .await;
    let info = result.get("IxInfo").expect("IxInfo variant");
    let ix = info.get("ix").expect("ix slice");
    assert_eq!(
        ix.get("name").and_then(|v| v.as_str()),
        Some("switch_power")
    );
    let disc = ix
        .get("discriminator_hex")
        .and_then(|v| v.as_str())
        .expect("discriminator_hex");
    assert!(disc.starts_with("0x") && disc.len() == 18);
    let args = ix
        .get("args")
        .and_then(|v| v.as_array())
        .expect("args array");
    assert_eq!(args.len(), 1);
    assert_eq!(
        args[0].get("ty").and_then(|v| v.as_str()),
        Some("string")
    );
    assert_eq!(
        info.get("admin_gated").and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[tokio::test]
async fn solana_coupling_returns_pairs() {
    let (client, port) = start_solana().await;
    let result = cmd(&client, port, "lever", serde_json::json!("Coupling")).await;
    let pairs = result
        .get("CouplingList")
        .and_then(|v| v.get("pairs"))
        .and_then(|v| v.as_array())
        .expect("CouplingList.pairs");
    // lever has 2 ixs both writing `power` → exactly one pair.
    assert_eq!(pairs.len(), 1);
    let only = &pairs[0];
    assert_eq!(only.get("a").and_then(|v| v.as_str()), Some("initialize"));
    assert_eq!(
        only.get("b").and_then(|v| v.as_str()),
        Some("switch_power")
    );
}

#[tokio::test]
async fn solana_vars_returns_account_types() {
    let (client, port) = start_solana().await;
    let result = cmd(&client, port, "lever", serde_json::json!("Vars")).await;
    let accounts = result
        .get("AccountTypes")
        .and_then(|v| v.get("accounts"))
        .and_then(|v| v.as_array())
        .expect("AccountTypes.accounts");
    assert_eq!(accounts.len(), 1);
    assert_eq!(
        accounts[0].get("name").and_then(|v| v.as_str()),
        Some("PowerStatus")
    );
    let fields = accounts[0]
        .get("fields")
        .and_then(|v| v.as_array())
        .expect("fields");
    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[0].get("ty").and_then(|v| v.as_str()),
        Some("bool")
    );
}
