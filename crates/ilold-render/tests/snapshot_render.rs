//! Byte-identity snapshots for `render_solana_result`. ANSI escapes are
//! stripped before comparison so the snapshots stay portable across
//! terminals. To regenerate after an intentional render change, run
//! `ILOLD_UPDATE_SNAPSHOTS=1 cargo test -p ilold-render --test snapshot_render`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ilold_render::fmt::strip_ansi;
use ilold_render::render_solana_result;
use ilold_solana_core::exploration::SolanaCommandResult;
use ilold_solana_core::overlay::{CpiEdge, CuStats, RuntimeOverlay};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = snapshot_dir().join(format!("{name}.txt"));
    if std::env::var("ILOLD_UPDATE_SNAPSHOTS").is_ok() || !path.exists() {
        fs::create_dir_all(path.parent().unwrap()).expect("create snapshot dir");
        fs::write(&path, actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).expect("read snapshot");
    assert_eq!(
        actual, expected,
        "snapshot drift for {name}: regenerate with ILOLD_UPDATE_SNAPSHOTS=1"
    );
}

fn step_added_fixture() -> SolanaCommandResult {
    SolanaCommandResult::StepAdded {
        step_index: 2,
        instruction: "stake".into(),
        logs_excerpt: vec![
            "Program log: Instruction: Stake".into(),
            "Program log: amount=1000".into(),
        ],
        account_diffs_count: 3,
        compute_units: 12_345,
        error: None,
    }
}

fn coverage_fixture() -> SolanaCommandResult {
    let mut calls = BTreeMap::new();
    calls.insert("stake".into(), 4u32);
    calls.insert("claim_rewards".into(), 2u32);
    let mut failed = BTreeMap::new();
    failed.insert("claim_rewards".into(), 1u32);
    let mut cu_stats = BTreeMap::new();
    cu_stats.insert(
        "stake".into(),
        CuStats { min: 10_000, max: 15_000, avg: 12_500, samples: 4 },
    );
    cu_stats.insert(
        "claim_rewards".into(),
        CuStats { min: 8_000, max: 9_500, avg: 8_750, samples: 2 },
    );
    let overlay = RuntimeOverlay {
        program: "staking".into(),
        scenario: "main".into(),
        calls_per_ix: calls,
        failed_per_ix: failed,
        cu_stats_per_ix: cu_stats,
        cpi_edges: vec![CpiEdge {
            from_ix: "stake".into(),
            to_program: "token_program".into(),
            depth: 1,
            samples: 4,
        }],
    };
    SolanaCommandResult::Coverage { overlay }
}

fn error_fixture() -> SolanaCommandResult {
    SolanaCommandResult::Error {
        message: "scenario 'foo' not found".into(),
    }
}

#[test]
fn snapshot_step_added() {
    let out = strip_ansi(&render_solana_result(&step_added_fixture()));
    assert_snapshot("step_added", &out);
}

#[test]
fn snapshot_coverage() {
    let out = strip_ansi(&render_solana_result(&coverage_fixture()));
    assert_snapshot("coverage", &out);
}

#[test]
fn snapshot_error() {
    let out = strip_ansi(&render_solana_result(&error_fixture()));
    assert_snapshot("error", &out);
}
