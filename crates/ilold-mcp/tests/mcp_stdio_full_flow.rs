//! End-to-end stdio integration test for `ilold mcp`.
//!
//! Boots a mocked Ilold backend with `httpmock`, spawns the real `ilold mcp`
//! binary as a child process, exchanges the MCP initialize handshake,
//! lists tools, and finally fires a `tools/call ilold_funcs` and validates
//! that the structured `InstructionList` payload comes back.
//!
//! Marked `#[ignore]`: requires the `ilold` binary to be pre-built (the test
//! locates it at `<workspace>/target/<profile>/ilold`). Run it after
//! `cargo build -p ilold-cli` (debug) or `cargo build --release -p ilold-cli`.
//! `cargo test --workspace` skips it by default to keep CI fast.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use serde_json::{Value, json};

fn ilold_binary() -> Option<PathBuf> {
    // Two candidates: debug build, then release.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent()?.parent()?;
    for profile in ["debug", "release"] {
        let p = workspace.join("target").join(profile).join("ilold");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn write_json_line<W: Write>(w: &mut W, v: &Value) -> std::io::Result<()> {
    let line = serde_json::to_string(v).expect("serialize json");
    writeln!(w, "{line}")?;
    w.flush()
}

fn read_response<R: BufRead>(reader: &mut R, want_id: u64) -> Option<Value> {
    for _ in 0..256 {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if parsed.get("id").and_then(|v| v.as_u64()) == Some(want_id) {
            return Some(parsed);
        }
    }
    None
}

#[test]
#[ignore = "requires pre-built `ilold` binary at target/debug/ilold or target/release/ilold"]
fn mcp_stdio_full_flow_initialize_list_call() {
    let ilold = match ilold_binary() {
        Some(p) => p,
        None => {
            eprintln!("skipping: ilold binary not found; run `cargo build -p ilold-cli` first");
            return;
        }
    };

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/project/map");
        then.status(200).json_body(json!({
            "kind": "solana",
            "programs": [{ "name": "staking" }]
        }));
    });
    server.mock(|when, then| {
        when.method(POST).path("/api/cmd");
        then.status(200).json_body(json!({
            "InstructionList": {
                "items": [
                    {
                        "name": "stake",
                        "args_count": 1,
                        "accounts_count": 3,
                        "has_pdas": false,
                        "signers": ["user"]
                    },
                    {
                        "name": "unstake",
                        "args_count": 0,
                        "accounts_count": 2,
                        "has_pdas": false,
                        "signers": ["user"]
                    }
                ]
            }
        }));
    });

    let mut child = Command::new(&ilold)
        .arg("mcp")
        .arg("--server-url")
        .arg(server.base_url())
        .arg("--contract")
        .arg("staking")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ilold mcp");

    let mut stdin = child.stdin.take().expect("stdin handle");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout handle"));

    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ilold-integration-test", "version": "0.0.0" }
        }
    });
    write_json_line(&mut stdin, &init).expect("send initialize");
    let init_resp = read_response(&mut stdout, 1).expect("initialize response");
    assert!(
        init_resp.get("result").is_some(),
        "initialize must succeed: {init_resp}"
    );

    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    write_json_line(&mut stdin, &initialized).expect("send initialized notif");

    let list = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    write_json_line(&mut stdin, &list).expect("send tools/list");
    let list_resp = read_response(&mut stdout, 2).expect("tools/list response");
    let tools = list_resp
        .pointer("/result/tools")
        .and_then(|v| v.as_array())
        .cloned()
        .expect("tools array");
    assert_eq!(tools.len(), 30, "expected 30 tools, got {}", tools.len());
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(names.contains(&"ilold_funcs"));
    assert!(names.contains(&"ilold_call"));

    let call = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": { "name": "ilold_funcs", "arguments": {} }
    });
    write_json_line(&mut stdin, &call).expect("send tools/call");
    let call_resp = read_response(&mut stdout, 3).expect("tools/call response");

    let structured = call_resp
        .pointer("/result/structuredContent")
        .expect("structuredContent present");
    let items = structured
        .pointer("/InstructionList/items")
        .and_then(|v| v.as_array())
        .expect("InstructionList.items present");
    assert_eq!(items.len(), 2);

    let text = call_resp
        .pointer("/result/content/0/text")
        .and_then(|v| v.as_str())
        .expect("text content");
    assert!(
        text.contains("stake"),
        "expected rendered text to mention `stake`, got: {text}"
    );

    drop(stdin);
    let _ = child.wait_timeout_or_kill(Duration::from_secs(5));
}

trait WaitTimeoutExt {
    fn wait_timeout_or_kill(&mut self, timeout: Duration);
}

impl WaitTimeoutExt for std::process::Child {
    fn wait_timeout_or_kill(&mut self, timeout: Duration) {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            match self.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        let _ = self.kill();
        let _ = self.wait();
    }
}
