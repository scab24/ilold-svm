//! Integration tests for `GET /api/program/:name/:ix/source`.
//!
//! Mirrors `contract_source_test.rs` (Solidity) but boots a Solana server
//! against the staking Anchor fixture.

use std::path::PathBuf;

use ilold_solana_core::ingest::detect;

fn staking_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/solana/staking")
}

async fn start_staking() -> (reqwest::Client, u16) {
    let detected = detect(&staking_fixture()).expect("detect staking fixture");
    let (_state, port) = ilold_web::start_solana_server(detected, 0)
        .await
        .expect("start solana server");
    (reqwest::Client::new(), port)
}

#[tokio::test]
async fn get_instruction_source_returns_source_for_stake() {
    let (client, port) = start_staking().await;

    let res = client
        .get(format!(
            "http://127.0.0.1:{port}/api/program/staking/stake/source"
        ))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "status: {}", res.status());

    let body: serde_json::Value = res.json().await.unwrap();
    let source = body.get("source").and_then(|v| v.as_str()).unwrap();
    assert!(
        source.contains("pub fn stake"),
        "expected handler signature, got:\n{source}"
    );
    assert!(
        source.contains("StakingError::ZeroAmount"),
        "expected handler body, got:\n{source}"
    );

    let start_line = body
        .pointer("/span/start_line")
        .and_then(|v| v.as_u64())
        .unwrap();
    let end_line = body
        .pointer("/span/end_line")
        .and_then(|v| v.as_u64())
        .unwrap();
    assert!(
        end_line > start_line,
        "handler span must cover multiple lines, got {start_line}..{end_line}"
    );

    let file_path = body.get("file_path").and_then(|v| v.as_str()).unwrap();
    assert!(
        std::path::Path::new(file_path).is_absolute(),
        "file_path must be absolute, got: {file_path}"
    );
    assert!(
        file_path.ends_with("programs/staking/src/lib.rs"),
        "unexpected file_path: {file_path}"
    );
}

#[tokio::test]
async fn get_instruction_source_returns_404_for_missing_ix() {
    let (client, port) = start_staking().await;

    let res = client
        .get(format!(
            "http://127.0.0.1:{port}/api/program/staking/does_not_exist/source"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        reqwest::StatusCode::NOT_FOUND,
        "unknown instruction must 404, got: {}",
        res.status(),
    );
}

#[tokio::test]
async fn get_instruction_source_returns_404_for_unknown_program() {
    let (client, port) = start_staking().await;

    let res = client
        .get(format!(
            "http://127.0.0.1:{port}/api/program/nope/stake/source"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        reqwest::StatusCode::NOT_FOUND,
        "unknown program must 404, got: {}",
        res.status(),
    );
}
