use std::collections::HashMap;

use ilold_session_core::exploration::session::{
    ExplorationSession, ExplorationStep, MutationScope, StateMutation, TraceConfig,
};
use ilold_session_core::journal::types::JournalEntry;

use crate::cfg::types::CfgGraph;
use crate::model::common::StateVar;
use crate::model::contract::ContractDef;
use crate::model::function::FunctionDef;
use crate::model::project::Project;
use crate::narrative::trace::{build_flow_tree_with_mutations, FlowConfig};

#[allow(clippy::too_many_arguments)]
pub fn add_solidity_step<'a>(
    session: &'a mut ExplorationSession,
    function: &FunctionDef,
    cfg: &CfgGraph,
    state_vars: &[StateVar],
    project: &Project,
    owning_contract: &ContractDef,
    all_cfgs: &HashMap<(String, String), CfgGraph>,
    timestamp: &str,
    trace_config: TraceConfig,
) -> &'a ExplorationStep {
    let step_index = session.steps.len();

    let flow_config = FlowConfig {
        max_depth: trace_config.depth,
        include_reverts: trace_config.include_reverts,
        expand_set: trace_config.expand_set.iter().copied().collect(),
    };

    let (flow_tree, raw_mutations) = build_flow_tree_with_mutations(
        owning_contract,
        function,
        cfg,
        project,
        all_cfgs,
        &flow_config,
    );

    let mutations: Vec<StateMutation> = raw_mutations
        .into_iter()
        .filter_map(|fm| {
            let base = crate::util::target_base_name(&fm.target);
            if !state_vars.iter().any(|sv| sv.name == base) {
                return None;
            }
            Some(StateMutation {
                variable: fm.target,
                operator: fm.operator,
                value_expr: fm.value,
                step_index,
                via: fm.via,
                flow_step_id: Some(fm.flow_step_id),
                scope: MutationScope::State,
            })
        })
        .collect();

    let flow_tree_value = serde_json::to_value(&flow_tree).ok();

    session.steps.push(ExplorationStep {
        function: function.name.clone(),
        mutations,
        flow_tree: flow_tree_value,
        trace_config,
        runtime_trace: None,
            call_payload: None,
    });

    session.journal.record(JournalEntry::SequenceExplored {
        steps: session.steps.iter().map(|s| s.function.clone()).collect(),
        timestamp: timestamp.into(),
    });

    session.steps.last().unwrap()
}
