//! Integration tests for scenario lifecycle commands (Phase S2 / Task 2.12).
//!
//! Each test starts a fresh server on port 0 (parallel-safe) and POSTs
//! commands through `/api/cmd`, asserting on the JSON response body. The
//! helper `cmd()` keeps each test body focused on the scenario semantics
//! rather than HTTP plumbing.

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

async fn start() -> (reqwest::Client, u16) {
    let paths = vec![fixture("staking.sol")];
    let (_state, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();
    (reqwest::Client::new(), port)
}

async fn cmd(
    client: &reqwest::Client,
    port: u16,
    contract: &str,
    command: serde_json::Value,
) -> serde_json::Value {
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
    res.json().await.expect("response was not JSON")
}

fn scenario_new(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "New": { "name": name } } } })
}

fn scenario_list() -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": "List" } })
}

fn scenario_switch(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "Switch": { "name": name } } } })
}

fn scenario_fork(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "Fork": { "name": name } } } })
}

fn scenario_fork_at(name: &str, at_step: usize) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "Fork": { "name": name, "at_step": at_step } } } })
}

fn scenario_delete(name: &str) -> serde_json::Value {
    serde_json::json!({ "Scenario": { "sub": { "Delete": { "name": name } } } })
}

fn call(func: &str) -> serde_json::Value {
    serde_json::json!({ "Call": { "func": func } })
}

/// Extract the ScenarioList items from a CommandResult JSON body.
fn list_items(r: &serde_json::Value) -> &Vec<serde_json::Value> {
    r.get("ScenarioList")
        .and_then(|v| v.get("items"))
        .and_then(|v| v.as_array())
        .expect("ScenarioList.items array")
}

fn find_scenario<'a>(items: &'a [serde_json::Value], name: &str) -> &'a serde_json::Value {
    items
        .iter()
        .find(|it| it.get("name").and_then(|n| n.as_str()) == Some(name))
        .unwrap_or_else(|| panic!("scenario '{name}' not in list: {items:?}"))
}

#[tokio::test]
async fn scenario_new_creates_empty_scenario() {
    let (client, port) = start().await;

    let r = cmd(&client, port, "Staking", scenario_new("alt1")).await;
    assert_eq!(
        r.get("ScenarioCreated")
            .and_then(|v| v.get("name"))
            .and_then(|n| n.as_str()),
        Some("alt1"),
        "expected ScenarioCreated{{name: alt1}}, got: {r}"
    );

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(items.len(), 2, "expected 2 scenarios, got: {items:?}");
    let main = find_scenario(items, "main");
    assert_eq!(main.get("active").and_then(|v| v.as_bool()), Some(true));
    let alt1 = find_scenario(items, "alt1");
    assert_eq!(alt1.get("active").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(
        alt1.get("step_count").and_then(|v| v.as_u64()),
        Some(0),
        "alt1 should start empty"
    );
}

#[tokio::test]
async fn scenario_list_shows_all_with_active_marker() {
    let (client, port) = start().await;

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(items.len(), 1);
    let main = find_scenario(items, "main");
    assert_eq!(main.get("active").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(main.get("step_count").and_then(|v| v.as_u64()), Some(0));

    cmd(&client, port, "Staking", call("deposit")).await;

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    let main = find_scenario(items, "main");
    assert_eq!(
        main.get("step_count").and_then(|v| v.as_u64()),
        Some(1),
        "main should have 1 step after deposit"
    );
}

#[tokio::test]
async fn scenario_switch_changes_active() {
    let (client, port) = start().await;

    cmd(&client, port, "Staking", scenario_new("alt1")).await;

    let r = cmd(&client, port, "Staking", scenario_switch("alt1")).await;
    let switched = r.get("ScenarioSwitched").expect("ScenarioSwitched variant");
    assert_eq!(
        switched.get("from").and_then(|v| v.as_str()),
        Some("main")
    );
    assert_eq!(switched.get("to").and_then(|v| v.as_str()), Some("alt1"));

    cmd(&client, port, "Staking", call("deposit")).await;

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    let alt1 = find_scenario(items, "alt1");
    assert_eq!(alt1.get("active").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(alt1.get("step_count").and_then(|v| v.as_u64()), Some(1));
    let main = find_scenario(items, "main");
    assert_eq!(main.get("active").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(main.get("step_count").and_then(|v| v.as_u64()), Some(0));
}

#[tokio::test]
async fn scenario_fork_clones_prefix_at_current_step() {
    let (client, port) = start().await;

    cmd(&client, port, "Staking", call("deposit")).await;

    let r = cmd(&client, port, "Staking", scenario_fork("alt1")).await;
    let forked = r.get("ScenarioForked").expect("ScenarioForked variant");
    assert_eq!(forked.get("from").and_then(|v| v.as_str()), Some("main"));
    assert_eq!(forked.get("to").and_then(|v| v.as_str()), Some("alt1"));
    assert_eq!(forked.get("at_step").and_then(|v| v.as_u64()), Some(1));

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(
        find_scenario(items, "main")
            .get("step_count")
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        find_scenario(items, "alt1")
            .get("step_count")
            .and_then(|v| v.as_u64()),
        Some(1),
        "fork should copy the prefix"
    );
}

#[tokio::test]
async fn scenario_fork_emits_branch_created_journal_entry() {
    let (client, port) = start().await;

    cmd(&client, port, "Staking", call("deposit")).await;
    cmd(&client, port, "Staking", scenario_fork("alt1")).await;
    cmd(&client, port, "Staking", scenario_switch("alt1")).await;

    // SaveSession returns the serialized session; the journal.entries array
    // is the source of truth for BranchCreated.
    let r = cmd(
        &client,
        port,
        "Staking",
        serde_json::json!("SaveSession"),
    )
    .await;
    let json_str = r
        .get("SessionSaved")
        .and_then(|v| v.get("json"))
        .and_then(|v| v.as_str())
        .expect("SessionSaved.json");

    let saved: serde_json::Value =
        serde_json::from_str(json_str).expect("SaveSession payload was not JSON");
    let entries = saved
        .get("journal")
        .and_then(|j| j.get("entries"))
        .and_then(|e| e.as_array())
        .expect("journal.entries array");

    let branch = entries
        .iter()
        .find_map(|e| e.get("BranchCreated"))
        .unwrap_or_else(|| panic!("no BranchCreated entry in {entries:?}"));
    assert_eq!(
        branch.get("from_function").and_then(|v| v.as_str()),
        Some("main")
    );
    assert_eq!(
        branch.get("branch_function").and_then(|v| v.as_str()),
        Some("alt1")
    );
}

#[tokio::test]
async fn scenario_delete_refuses_when_active() {
    let (client, port) = start().await;

    // `main` is active by default — deletion must fail.
    let r = cmd(&client, port, "Staking", scenario_delete("main")).await;
    let msg = r
        .get("Error")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_str())
        .expect("Error.message");
    assert!(
        msg.contains("Cannot delete active scenario"),
        "unexpected error message: {msg}"
    );
}

/// The "only remaining" branch of `execute_scenario` is defensive — it is
/// unreachable via HTTP because, when only one scenario exists, it is by
/// definition the active one and the "active" check fires first. We still
/// verify that calling delete on the sole `main` scenario surfaces the
/// active-guard error (not a panic or a "does not exist" error). The internal
/// `ScenarioStore::remove` path for the only-remaining case is exercised by
/// unit tests in the core crate.
#[tokio::test]
async fn scenario_delete_only_remaining_is_guarded_by_active_check() {
    let (client, port) = start().await;

    let r = cmd(&client, port, "Staking", scenario_delete("main")).await;
    let msg = r
        .get("Error")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_str())
        .expect("Error.message");
    assert!(
        msg.contains("Cannot delete active scenario"),
        "expected active-guard error, got: {msg}"
    );
}

#[tokio::test]
async fn scenario_delete_removes_other_scenario() {
    let (client, port) = start().await;

    cmd(&client, port, "Staking", scenario_new("alt1")).await;
    cmd(&client, port, "Staking", scenario_new("alt2")).await;
    cmd(&client, port, "Staking", scenario_switch("alt1")).await;

    let r = cmd(&client, port, "Staking", scenario_delete("alt2")).await;
    assert_eq!(
        r.get("ScenarioDeleted")
            .and_then(|v| v.get("name"))
            .and_then(|n| n.as_str()),
        Some("alt2"),
        "expected ScenarioDeleted{{name: alt2}}, got: {r}"
    );

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(items.len(), 2);
    assert_eq!(
        find_scenario(items, "alt1")
            .get("active")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        find_scenario(items, "main")
            .get("active")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[tokio::test]
async fn scenario_name_validation_rejects_invalid() {
    let (client, port) = start().await;

    for bad in [
        "InvalidName!",
        "1starts-with-digit",
        "",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        let r = cmd(&client, port, "Staking", scenario_new(bad)).await;
        let msg = r
            .get("Error")
            .and_then(|v| v.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("expected Error for name {bad:?}, got: {r}"));
        assert!(
            msg.contains("Invalid scenario name"),
            "unexpected error for {bad:?}: {msg}"
        );
    }
}

// ── Phase S7: fork-at-step-N ────────────────────────────────────────────────

#[tokio::test]
async fn scenario_fork_at_step_truncates_to_n() {
    let (client, port) = start().await;

    // Build main with 3 steps, then fork at step 2.
    cmd(&client, port, "Staking", call("deposit")).await;
    cmd(&client, port, "Staking", call("deposit")).await;
    cmd(&client, port, "Staking", call("withdraw")).await;

    let r = cmd(&client, port, "Staking", scenario_fork_at("alt1", 2)).await;
    let forked = r.get("ScenarioForked").expect("ScenarioForked variant");
    assert_eq!(forked.get("at_step").and_then(|v| v.as_u64()), Some(2));

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(
        find_scenario(items, "main")
            .get("step_count")
            .and_then(|v| v.as_u64()),
        Some(3),
        "main must be unchanged"
    );
    assert_eq!(
        find_scenario(items, "alt1")
            .get("step_count")
            .and_then(|v| v.as_u64()),
        Some(2),
        "alt1 must be truncated to first 2 steps"
    );
}

#[tokio::test]
async fn scenario_fork_at_step_zero_creates_empty() {
    let (client, port) = start().await;

    cmd(&client, port, "Staking", call("deposit")).await;
    cmd(&client, port, "Staking", call("withdraw")).await;

    let r = cmd(&client, port, "Staking", scenario_fork_at("alt1", 0)).await;
    assert_eq!(
        r.get("ScenarioForked")
            .and_then(|v| v.get("at_step"))
            .and_then(|v| v.as_u64()),
        Some(0)
    );

    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(
        find_scenario(items, "alt1")
            .get("step_count")
            .and_then(|v| v.as_u64()),
        Some(0),
        "fork at 0 must yield an empty scenario"
    );
}

#[tokio::test]
async fn scenario_fork_at_step_greater_than_length_errors() {
    let (client, port) = start().await;

    cmd(&client, port, "Staking", call("deposit")).await;

    let r = cmd(&client, port, "Staking", scenario_fork_at("alt1", 5)).await;
    let msg = r
        .get("Error")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("expected Error, got: {r}"));
    assert!(
        msg.contains("Cannot fork at step 5") && msg.contains("only 1 step"),
        "unexpected error: {msg}"
    );

    // Verify no scenario was created on failure.
    let r = cmd(&client, port, "Staking", scenario_list()).await;
    let items = list_items(&r);
    assert_eq!(items.len(), 1, "failed fork must not create a scenario");
}

#[tokio::test]
async fn scenario_name_reserved_main_cannot_be_recreated() {
    let (client, port) = start().await;

    // `main` is auto-seeded on startup, so `scenario new main` collides on
    // the duplicate check.
    let r = cmd(&client, port, "Staking", scenario_new("main")).await;
    let msg = r
        .get("Error")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("expected Error, got: {r}"));
    assert!(
        msg.contains("already exists"),
        "unexpected error message: {msg}"
    );
}
