use std::collections::BTreeMap;

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_session_core::runtime_trace::RuntimeTrace;
use serde::{Deserialize, Serialize};

/// Extract the deduplicated, insertion-ordered list of CPI program IDs
/// invoked by a RuntimeTrace. Lifted out so both the overlay aggregator
/// and the WS broadcast site share one decoder for `inner_instructions`.
/// Order mirrors `inner_instructions` (first hit wins) so the resulting
/// list reflects the CPI call sequence the program actually emitted.
pub fn extract_cpi_programs(trace: &RuntimeTrace) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for ii in &trace.inner_instructions {
        if seen.insert(ii.program.clone()) {
            out.push(ii.program.clone());
        }
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RuntimeOverlay {
    pub program: String,
    pub scenario: String,
    pub calls_per_ix: BTreeMap<String, u32>,
    pub failed_per_ix: BTreeMap<String, u32>,
    pub cu_stats_per_ix: BTreeMap<String, CuStats>,
    pub cpi_edges: Vec<CpiEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CuStats {
    pub min: u64,
    pub max: u64,
    pub avg: u64,
    pub samples: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpiEdge {
    pub from_ix: String,
    pub to_program: String,
    pub depth: u32,
    pub samples: u32,
}

impl RuntimeOverlay {
    pub fn from_session(session: &ExplorationSession) -> Self {
        let mut overlay = RuntimeOverlay {
            program: session.contract.clone(),
            scenario: String::new(),
            calls_per_ix: BTreeMap::new(),
            failed_per_ix: BTreeMap::new(),
            cu_stats_per_ix: BTreeMap::new(),
            cpi_edges: Vec::new(),
        };

        let mut cu_samples: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        // Aggregation key for CPI edges: (from_ix, to_program, depth). Tracked
        // separately so the final Vec<CpiEdge> can be deterministic-sorted.
        let mut cpi_counts: BTreeMap<(String, String, u32), u32> = BTreeMap::new();

        for step in &session.steps {
            *overlay.calls_per_ix.entry(step.function.clone()).or_insert(0) += 1;

            let trace: Option<RuntimeTrace> = step
                .runtime_trace
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            if let Some(t) = trace.as_ref() {
                cu_samples
                    .entry(step.function.clone())
                    .or_default()
                    .push(t.compute_units);

                for inner in &t.inner_instructions {
                    let key = (
                        step.function.clone(),
                        inner.program.clone(),
                        inner.depth,
                    );
                    *cpi_counts.entry(key).or_insert(0) += 1;
                }
            }
        }

        for (ix, count) in &session.failed_calls_per_ix {
            overlay.failed_per_ix.insert(ix.clone(), *count);
        }

        for (ix, samples) in cu_samples {
            let count = samples.len() as u32;
            if count == 0 {
                continue;
            }
            let min = samples.iter().copied().min().unwrap_or(0);
            let max = samples.iter().copied().max().unwrap_or(0);
            let sum: u128 = samples.iter().map(|v| *v as u128).sum();
            let avg = (sum / count as u128) as u64;
            overlay
                .cu_stats_per_ix
                .insert(ix, CuStats { min, max, avg, samples: count });
        }

        let mut edges: Vec<CpiEdge> = cpi_counts
            .into_iter()
            .map(|((from_ix, to_program, depth), samples)| CpiEdge {
                from_ix,
                to_program,
                depth,
                samples,
            })
            .collect();
        edges.sort_by(|a, b| {
            a.from_ix
                .cmp(&b.from_ix)
                .then_with(|| a.to_program.cmp(&b.to_program))
                .then_with(|| a.depth.cmp(&b.depth))
        });
        overlay.cpi_edges = edges;

        overlay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ilold_session_core::exploration::session::{
        ExplorationSession, ExplorationStep, TraceConfig,
    };
    use ilold_session_core::runtime_trace::{InnerInstruction, RuntimeTrace};

    fn empty_session() -> ExplorationSession {
        ExplorationSession::new("staking", "ilold")
    }

    fn step_with_trace(name: &str, trace: RuntimeTrace) -> ExplorationStep {
        ExplorationStep {
            function: name.to_string(),
            mutations: vec![],
            flow_tree: None,
            trace_config: TraceConfig::default(),
            runtime_trace: Some(serde_json::to_value(&trace).unwrap()),
            call_payload: None,
        }
    }

    fn ok_trace(cu: u64) -> RuntimeTrace {
        RuntimeTrace {
            logs: vec![],
            compute_units: cu,
            inner_instructions: vec![],
            account_diffs: vec![],
            return_data: None,
            error: None,
        }
    }

    #[test]
    fn from_session_empty_returns_empty() {
        let session = empty_session();
        let overlay = RuntimeOverlay::from_session(&session);
        assert_eq!(overlay.program, "staking");
        assert!(overlay.calls_per_ix.is_empty());
        assert!(overlay.failed_per_ix.is_empty());
        assert!(overlay.cu_stats_per_ix.is_empty());
        assert!(overlay.cpi_edges.is_empty());
    }

    #[test]
    fn from_session_aggregates_calls_per_ix() {
        let mut session = empty_session();
        session.steps.push(step_with_trace("stake", ok_trace(10_000)));
        session.steps.push(step_with_trace("stake", ok_trace(14_000)));
        session.steps.push(step_with_trace("unstake", ok_trace(12_000)));

        let overlay = RuntimeOverlay::from_session(&session);
        assert_eq!(overlay.calls_per_ix.get("stake").copied(), Some(2));
        assert_eq!(overlay.calls_per_ix.get("unstake").copied(), Some(1));
        assert!(overlay.failed_per_ix.is_empty());

        let stake = overlay.cu_stats_per_ix.get("stake").expect("stake stats");
        assert_eq!(stake.samples, 2);
        assert_eq!(stake.min, 10_000);
        assert_eq!(stake.max, 14_000);
        assert_eq!(stake.avg, 12_000);
    }

    #[test]
    fn from_session_reads_failed_calls_counter() {
        let mut session = empty_session();
        session.steps.push(step_with_trace("stake", ok_trace(11_000)));
        // Failed Calls never push a step (they go through record_failed_call
        // in execute_call::CallFailed). Mirror that real flow here.
        session.record_failed_call("stake");
        session.record_failed_call("unstake");

        let overlay = RuntimeOverlay::from_session(&session);
        assert_eq!(overlay.calls_per_ix.get("stake").copied(), Some(1));
        assert_eq!(overlay.failed_per_ix.get("stake").copied(), Some(1));
        assert_eq!(overlay.failed_per_ix.get("unstake").copied(), Some(1));
        assert_eq!(overlay.calls_per_ix.get("unstake"), None);
    }

    #[test]
    fn extract_cpi_programs_dedups_in_insertion_order() {
        let trace = RuntimeTrace {
            logs: vec![],
            compute_units: 0,
            inner_instructions: vec![
                InnerInstruction { program: "B".into(), instruction: "x".into(), depth: 1 },
                InnerInstruction { program: "A".into(), instruction: "x".into(), depth: 1 },
                InnerInstruction { program: "B".into(), instruction: "y".into(), depth: 2 },
                InnerInstruction { program: "A".into(), instruction: "z".into(), depth: 1 },
            ],
            account_diffs: vec![],
            return_data: None,
            error: None,
        };
        let programs = extract_cpi_programs(&trace);
        assert_eq!(programs, vec!["B".to_string(), "A".to_string()]);
    }

    #[test]
    fn extract_cpi_programs_empty_trace_is_empty() {
        let trace = ok_trace(1_000);
        assert!(extract_cpi_programs(&trace).is_empty());
    }

    #[test]
    fn from_session_collects_cpi_edges() {
        let mut session = empty_session();
        let mut trace_a = ok_trace(15_000);
        trace_a.inner_instructions = vec![
            InnerInstruction {
                program: "11111111111111111111111111111111".into(),
                instruction: "abc".into(),
                depth: 1,
            },
            InnerInstruction {
                program: "11111111111111111111111111111111".into(),
                instruction: "abc".into(),
                depth: 1,
            },
            InnerInstruction {
                program: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".into(),
                instruction: "xyz".into(),
                depth: 2,
            },
        ];
        let mut trace_b = ok_trace(13_000);
        trace_b.inner_instructions = vec![InnerInstruction {
            program: "11111111111111111111111111111111".into(),
            instruction: "abc".into(),
            depth: 1,
        }];
        session.steps.push(step_with_trace("stake", trace_a));
        session.steps.push(step_with_trace("unstake", trace_b));

        let overlay = RuntimeOverlay::from_session(&session);
        assert_eq!(overlay.cpi_edges.len(), 3);
        // Sorted by (from_ix, to_program, depth).
        let stake_sys = &overlay.cpi_edges[0];
        assert_eq!(stake_sys.from_ix, "stake");
        assert_eq!(stake_sys.to_program, "11111111111111111111111111111111");
        assert_eq!(stake_sys.depth, 1);
        assert_eq!(stake_sys.samples, 2);
        let stake_token = &overlay.cpi_edges[1];
        assert_eq!(stake_token.from_ix, "stake");
        assert_eq!(
            stake_token.to_program,
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        );
        assert_eq!(stake_token.depth, 2);
        assert_eq!(stake_token.samples, 1);
        let unstake_sys = &overlay.cpi_edges[2];
        assert_eq!(unstake_sys.from_ix, "unstake");
        assert_eq!(unstake_sys.samples, 1);
    }
}
