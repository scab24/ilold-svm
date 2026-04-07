use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("tests/fixtures")
        .join(name)
}

#[derive(Debug, Clone)]
struct FlowEvent {
    variant: String,
    payload: serde_json::Value,
}

fn flatten_flow_tree(node: &serde_json::Value, out: &mut Vec<FlowEvent>) {
    if let Some(kind) = node.get("kind") {
        match kind {
            serde_json::Value::String(s) => {
                out.push(FlowEvent { variant: s.clone(), payload: serde_json::Value::Null });
            }
            serde_json::Value::Object(map) => {
                if let Some((variant, payload)) = map.iter().next() {
                    out.push(FlowEvent { variant: variant.clone(), payload: payload.clone() });
                }
            }
            _ => {}
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            flatten_flow_tree(child, out);
        }
    }
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

#[tokio::test]
async fn trace_swap_shows_external_call_before_update_writes() {
    let paths = vec![fixture("uniswap_v2_pair.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/trace/UniswapV2Pair/swap?depth=4"))
        .send().await.unwrap();

    assert!(res.status().is_success(), "trace endpoint failed: {}", res.status());
    let tree: serde_json::Value = res.json().await.unwrap();

    assert_eq!(tree["contract"], "UniswapV2Pair");
    assert_eq!(tree["function"], "swap");
    assert_eq!(tree["max_depth"], 4);
    let modifiers = tree["modifiers"].as_array().unwrap();
    assert!(modifiers.iter().any(|m| m == "lock"));

    let mut events: Vec<FlowEvent> = Vec::new();
    flatten_flow_tree(&tree["root"], &mut events);

    let first_external = events.iter().position(|e| {
        e.variant == "ExternalCall"
            && e.payload.get("function").and_then(|v| v.as_str()) == Some("transfer")
    });
    assert!(first_external.is_some(), "Expected IERC20.transfer ExternalCall");

    assert!(
        events.iter().any(|e| {
            e.variant == "InternalCall"
                && e.payload.get("function").and_then(|v| v.as_str()) == Some("_update")
        }),
        "Expected _update as InternalCall",
    );

    assert!(
        events.iter().any(|e| {
            e.variant == "Write"
                && e.payload.get("target").and_then(|v| v.as_str()) == Some("reserve0")
        }),
        "Expected inlined write to reserve0",
    );

    // CEI order: a reserve0 write must exist after the first external transfer.
    let ext_idx = first_external.unwrap();
    assert!(
        events[ext_idx..].iter().any(|e| {
            e.variant == "Write"
                && e.payload.get("target").and_then(|v| v.as_str()) == Some("reserve0")
        }),
        "Expected reserve0 write AFTER first transfer (CEI). first_external={}",
        ext_idx,
    );
}

#[tokio::test]
async fn trace_getreserves_has_no_empty_target_writes() {
    // Regression: `return (reserve0, reserve1, blockTimestampLast)` used to
    // become an Assignment with an empty target because the classify_expression
    // catchall emitted fake Assignments for any non-call, non-assign expression.
    let paths = vec![fixture("uniswap_v2_pair.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/trace/UniswapV2Pair/getReserves"))
        .send().await.unwrap();

    assert!(res.status().is_success());
    let tree: serde_json::Value = res.json().await.unwrap();

    let mut events: Vec<FlowEvent> = Vec::new();
    flatten_flow_tree(&tree["root"], &mut events);

    for e in &events {
        if e.variant == "Write" {
            let target = e.payload.get("target").and_then(|v| v.as_str()).unwrap_or("");
            assert!(
                !target.is_empty(),
                "Write with empty target leaked into trace — classify_expression fallback regression: {:?}",
                e.payload,
            );
        }
    }

    // getReserves just reads state + returns. No writes expected at all.
    let write_count = events.iter().filter(|e| e.variant == "Write").count();
    assert_eq!(write_count, 0, "getReserves should emit no Write events");
}


#[tokio::test]
async fn trace_update_has_no_internal_calls_and_shows_state_writes() {
    let paths = vec![fixture("uniswap_v2_pair.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/trace/UniswapV2Pair/_update"))
        .send().await.unwrap();

    assert!(res.status().is_success());
    let tree: serde_json::Value = res.json().await.unwrap();

    let mut events: Vec<FlowEvent> = Vec::new();
    flatten_flow_tree(&tree["root"], &mut events);

    let has_write_to = |name: &str| {
        events.iter().any(|e| {
            e.variant == "Write"
                && e.payload.get("target").and_then(|v| v.as_str()) == Some(name)
        })
    };

    // _update has 3 writes + 1 emit, no internal calls
    assert!(has_write_to("reserve0"), "expected write to reserve0");
    assert!(has_write_to("reserve1"), "expected write to reserve1");
    assert!(events.iter().any(|e| e.variant == "EmitEvent"));
    assert!(!events.iter().any(|e| e.variant == "InternalCall"));
}
