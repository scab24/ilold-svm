use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("tests/fixtures")
        .join(name)
}

#[tokio::test]
async fn vars_endpoint_returns_state_variables() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{port}/api/contract/Staking"))
        .send().await.unwrap();

    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();

    let vars = body["state_vars"].as_array().unwrap();
    assert!(!vars.is_empty(), "Should have state variables");

    let names: Vec<&str> = vars.iter()
        .filter_map(|v| v["name"].as_str())
        .collect();
    assert!(names.contains(&"balances"), "Should contain balances: {:?}", names);
    assert!(names.contains(&"totalStaked"), "Should contain totalStaked: {:?}", names);
    assert!(names.contains(&"rewardRate"), "Should contain rewardRate: {:?}", names);

    let balances = vars.iter().find(|v| v["name"] == "balances").unwrap();
    assert_eq!(balances["is_constant"], false);
    assert_eq!(balances["is_immutable"], false);
    assert!(balances["type_name"].as_str().is_some());
}

#[tokio::test]
async fn info_endpoint_returns_narrative_with_tree_data() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    // Need an active session first
    client.post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}}))
        .send().await.unwrap();

    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/function/Staking/deposit"))
        .send().await.unwrap();

    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();

    assert_eq!(body["name"], "deposit");
    assert!(body["total_paths"].as_u64().unwrap() > 0);
    assert!(body["state_writes"].as_array().unwrap().len() > 0);
    assert!(body["modifiers"].as_array().unwrap().len() > 0);
    assert!(body["external_calls"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn sequence_endpoint_with_two_steps() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    // Build 2-step sequence
    client.post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}}))
        .send().await.unwrap();
    client.post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "withdraw"}}}))
        .send().await.unwrap();

    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/sequence"))
        .send().await.unwrap();

    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();

    assert!(body["steps"].as_array().unwrap().len() >= 2);
    assert!(body["observations"].as_array().is_some());
}

#[tokio::test]
async fn sequence_with_one_step_returns_error() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    client.post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}}))
        .send().await.unwrap();

    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/sequence"))
        .send().await.unwrap();

    assert_eq!(res.status(), 400);
    let body = res.text().await.unwrap();
    assert!(body.contains("at least 2 steps"));
}

#[tokio::test]
async fn full_explore_workflow() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();
    let cmd = |body: serde_json::Value| {
        let c = client.clone();
        let p = port;
        async move {
            c.post(format!("http://127.0.0.1:{p}/api/cmd"))
                .json(&body)
                .send().await.unwrap()
                .json::<serde_json::Value>().await.unwrap()
        }
    };

    // 1. Functions
    let res = cmd(serde_json::json!({"contract": "Staking", "command": "Functions"})).await;
    let funcs = res["FunctionList"]["functions"].as_array().unwrap();
    assert!(funcs.len() >= 8);

    // 2. Call deposit
    let res = cmd(serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}})).await;
    assert_eq!(res["StepAdded"]["function"], "deposit");
    assert_eq!(res["StepAdded"]["step_index"], 0);

    // 3. State after 1 step
    let res = cmd(serde_json::json!({"contract": "Staking", "command": "State"})).await;
    let summary = res["StateView"]["summary"].as_array().unwrap();
    assert!(!summary.is_empty());

    // 4. Who balances
    let res = cmd(serde_json::json!({"contract": "Staking", "command": {"Who": {"variable": "balances"}}})).await;
    let writers = res["VariableInfo"]["writers"].as_array().unwrap();
    assert!(writers.len() >= 2, "balances should have at least 2 writers");

    // 5. Call withdraw
    let res = cmd(serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "withdraw"}}})).await;
    assert_eq!(res["StepAdded"]["function"], "withdraw");
    assert_eq!(res["StepAdded"]["step_index"], 1);

    // 6. Back
    let res = cmd(serde_json::json!({"contract": "Staking", "command": "Back"})).await;
    assert_eq!(res["StepRemoved"]["remaining"], 1);

    // 7. Session
    let res = cmd(serde_json::json!({"contract": "Staking", "command": "Session"})).await;
    assert_eq!(res["SessionView"]["contract"], "Staking");
    let steps = res["SessionView"]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);

    // 8. Clear
    let res = cmd(serde_json::json!({"contract": "Staking", "command": "Clear"})).await;
    assert_eq!(res, serde_json::json!("Cleared"));

    // 9. Session after clear
    let res = cmd(serde_json::json!({"contract": "Staking", "command": "Session"})).await;
    assert_eq!(res["SessionView"]["steps"].as_array().unwrap().len(), 0);
}
