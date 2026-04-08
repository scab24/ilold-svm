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
    // Regression for classify_expression catchall emitting fake Assignments
    // for tuple returns. getReserves must have zero Write events.
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

/// Collect (step_id, variant) pairs from a FlowTree in pre-order.
fn collect_step_ids(node: &serde_json::Value, out: &mut Vec<(u64, String)>) {
    let step_id = node.get("step_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let variant = match node.get("kind") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Object(map)) => {
            map.keys().next().cloned().unwrap_or_default()
        }
        _ => String::new(),
    };
    out.push((step_id, variant));
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_step_ids(child, out);
        }
    }
}

/// Canonical step_ids must be stable across configs that only differ
/// in max_depth and expand_set. For every step_id present in BOTH trees,
/// the FlowKind variant must match.
#[tokio::test]
async fn step_ids_are_stable_across_max_depth_configs() {
    let paths = vec![fixture("uniswap_v2_pair.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();
    let get_tree = |depth: usize| {
        let client = client.clone();
        async move {
            let res = client
                .get(format!(
                    "http://127.0.0.1:{port}/api/session/trace/UniswapV2Pair/swap?depth={depth}"
                ))
                .send().await.unwrap();
            assert!(res.status().is_success());
            res.json::<serde_json::Value>().await.unwrap()
        }
    };

    let tree_shallow = get_tree(2).await;
    let tree_deep = get_tree(4).await;

    let mut shallow: Vec<(u64, String)> = Vec::new();
    collect_step_ids(&tree_shallow["root"], &mut shallow);
    let mut deep: Vec<(u64, String)> = Vec::new();
    collect_step_ids(&tree_deep["root"], &mut deep);

    // Both walks must visit a non-trivial tree.
    assert!(shallow.len() > 5, "shallow tree too small: {}", shallow.len());
    assert!(deep.len() > shallow.len(),
        "deep tree should have at least as many nodes as shallow (deep={}, shallow={})",
        deep.len(), shallow.len());

    // Build maps keyed by step_id.
    let shallow_map: std::collections::HashMap<u64, &str> =
        shallow.iter().map(|(id, v)| (*id, v.as_str())).collect();
    let deep_map: std::collections::HashMap<u64, &str> =
        deep.iter().map(|(id, v)| (*id, v.as_str())).collect();

    // Every step_id that exists in both trees must map to the same variant.
    let mut common_ids = 0usize;
    for (id, shallow_variant) in &shallow_map {
        if let Some(deep_variant) = deep_map.get(id) {
            assert_eq!(
                shallow_variant, deep_variant,
                "step_id {} has different variants: shallow={:?}, deep={:?}",
                id, shallow_variant, deep_variant,
            );
            common_ids += 1;
        }
    }
    assert!(common_ids > 5, "expected meaningful overlap, got {}", common_ids);
}

/// After `c <func>`, the persisted session must contain a non-null
/// flow_tree on the new step AND every harvested mutation must carry a
/// flow_step_id resolving to a Write/StateWrite node in that tree.
#[tokio::test]
async fn session_step_persists_flow_tree_with_populated_flow_step_ids() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    // 1. Add deposit to the session.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}}))
        .send().await.unwrap();
    assert!(res.status().is_success());

    // 2. Save the session via the SaveSession command to get its JSON form.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": "SaveSession"}))
        .send().await.unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    let json_str = body["SessionSaved"]["json"].as_str()
        .expect("SaveSession should return a JSON string");
    let session: serde_json::Value = serde_json::from_str(json_str).unwrap();

    // 3. Inspect the persisted step.
    let steps = session["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    let step = &steps[0];

    // 3a. flow_tree must be non-null.
    assert!(
        !step["flow_tree"].is_null(),
        "step.flow_tree must be persisted (not null) after c <func>"
    );
    let flow_tree = &step["flow_tree"];
    assert_eq!(flow_tree["function"], "deposit");
    let root = &flow_tree["root"];
    assert!(root["children"].as_array().unwrap().len() > 0,
        "persisted flow_tree must have non-empty root.children");

    // 3b. trace_config must be persisted with default depth.
    assert_eq!(step["trace_config"]["depth"], 2);

    // 3c. Every mutation must have flow_step_id = Some(_).
    let mutations = step["mutations"].as_array().unwrap();
    assert!(!mutations.is_empty(), "deposit should produce at least one mutation");
    for m in mutations {
        assert!(
            !m["flow_step_id"].is_null(),
            "mutation {:?} must have flow_step_id populated",
            m
        );
    }

    // 3d. Each flow_step_id must resolve to a Write or StateWrite node in the
    //     persisted flow_tree.
    let mut tree_step_ids: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
    collect_step_id_kinds(root, &mut tree_step_ids);
    for m in mutations {
        let id = m["flow_step_id"].as_u64().unwrap();
        let variant = tree_step_ids.get(&id)
            .unwrap_or_else(|| panic!("flow_step_id {} not found in persisted tree", id));
        assert!(
            variant == "Write" || variant == "StateWrite",
            "flow_step_id {} resolves to {}, expected Write or StateWrite",
            id, variant,
        );
    }
}

/// `GET /api/session/step/{N}/trace` returns the persisted FlowTree of
/// the session step. Verifies the new endpoint is wired correctly and
/// that the persisted tree round-trips through HTTP.
#[tokio::test]
async fn tr_step_endpoint_returns_persisted_flow_tree() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    // Add a step to the session.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}}))
        .send().await.unwrap();
    assert!(res.status().is_success());

    // GET the persisted trace.
    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/step/0/trace"))
        .send().await.unwrap();
    assert!(res.status().is_success(), "endpoint failed: {}", res.status());

    let tree: serde_json::Value = res.json().await.unwrap();
    assert_eq!(tree["function"], "deposit");
    assert_eq!(tree["max_depth"], 2);
    let root = &tree["root"];
    assert!(root["children"].as_array().unwrap().len() > 0);
}

/// Requesting an out-of-range step index returns 404.
#[tokio::test]
async fn tr_step_endpoint_404_on_unknown_step() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    // No steps added — index 0 doesn't exist.
    let res = client
        .get(format!("http://127.0.0.1:{port}/api/session/step/0/trace"))
        .send().await.unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
}

/// After `c <func>`, the `s` (state) command's variable summaries must
/// include `step N:flow_id` references in each change line, proving the
/// new walker's flow_step_id is propagating through to the render layer.
#[tokio::test]
async fn state_view_renders_flow_step_refs() {
    let paths = vec![fixture("staking.sol")];
    let (_, port) = ilold_web::start_server(paths, 0, 2).await.unwrap();

    let client = reqwest::Client::new();

    // Add a step.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": {"Call": {"func": "deposit"}}}))
        .send().await.unwrap();
    assert!(res.status().is_success());

    // Fetch the state view.
    let res = client
        .post(format!("http://127.0.0.1:{port}/api/cmd"))
        .json(&serde_json::json!({"contract": "Staking", "command": "State"}))
        .send().await.unwrap();
    assert!(res.status().is_success());
    let body: serde_json::Value = res.json().await.unwrap();
    let summary = body["StateView"]["summary"].as_array().unwrap();
    assert!(!summary.is_empty(), "deposit should produce state changes");

    // Every change line must carry a `step 0:` prefix followed by a digit
    // (the new format — `0:` is the session step, the digit is the
    // flow_step_id from the canonical walker).
    for var in summary {
        let changes = var["changes"].as_array().unwrap();
        for change in changes {
            let s = change.as_str().unwrap();
            let has_ref = s.split("step 0:").nth(1)
                .map(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .unwrap_or(false);
            assert!(
                has_ref,
                "change line missing 'step 0:N' flow ref: {:?}",
                s
            );
        }
    }
}

/// Collect (step_id, FlowKind variant) pairs from a serialized FlowTree.
fn collect_step_id_kinds(
    node: &serde_json::Value,
    out: &mut std::collections::HashMap<u64, String>,
) {
    if let Some(id) = node.get("step_id").and_then(|v| v.as_u64()) {
        let variant = match node.get("kind") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Object(map)) => {
                map.keys().next().cloned().unwrap_or_default()
            }
            _ => String::new(),
        };
        out.insert(id, variant);
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_step_id_kinds(child, out);
        }
    }
}
