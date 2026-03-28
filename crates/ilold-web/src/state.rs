use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use ilold_core::callgraph::builder::build_call_graph;
use ilold_core::callgraph::types::CallGraph;
use ilold_core::cfg::builder::CfgBuilder;
use ilold_core::cfg::types::CfgGraph;
use ilold_core::model::project::Project;
use ilold_core::parse::solar_frontend::SolarParser;
use ilold_core::parse::ProjectParser;
use ilold_core::pathtree::config::PruningConfig;
use ilold_core::pathtree::types::PathTree;
use ilold_core::pathtree::walker::build_path_tree;
use ilold_core::sequence::builder::build_sequence_tree;
use ilold_core::sequence::types::SequenceTree;

use serde::{Deserialize, Serialize};

/// All pre-computed analysis data, shared across REST and WebSocket handlers.
pub struct AppState {
    pub project: Project,
    pub cfgs: HashMap<(String, String), CfgGraph>,
    pub path_trees: HashMap<(String, String), PathTree>,
    pub call_graphs: HashMap<String, CallGraph>,
    pub sequence_trees: HashMap<String, SequenceTree>,
    pub annotations: RwLock<Vec<Annotation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub contract: String,
    pub function: Option<String>,
    pub path_id: Option<usize>,
    pub block_id: Option<usize>,
    pub text: String,
    pub severity: AnnotationSeverity,
    pub status: AnnotationStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationStatus {
    Open,
    Reviewed,
    Finding,
}

impl AppState {
    pub fn from_paths(paths: &[PathBuf], max_seq_depth: usize) -> anyhow::Result<Self> {
        let parser = SolarParser;
        let mut project = parser.parse(paths)?;
        project.rebuild_index();

        let config = PruningConfig::default();
        let mut cfgs = HashMap::new();
        let mut path_trees = HashMap::new();
        let mut call_graphs = HashMap::new();
        let mut sequence_trees = HashMap::new();

        for contract in &project.contracts {
            let cg = build_call_graph(&project, contract);
            call_graphs.insert(contract.name.clone(), cg);

            let mut contract_path_trees = Vec::new();

            for func in &contract.functions {
                let key = (contract.name.clone(), func.name.clone());

                if let Ok(cfg) = CfgBuilder::build(func, contract) {
                    let pt = build_path_tree(
                        &cfg,
                        &contract.name,
                        &func.name,
                        &contract.state_vars,
                        &config,
                    );
                    contract_path_trees.push(pt.clone());
                    path_trees.insert(key.clone(), pt);
                    cfgs.insert(key, cfg);
                }
            }

            let st = build_sequence_tree(contract, &contract_path_trees, max_seq_depth);
            sequence_trees.insert(contract.name.clone(), st);
        }

        Ok(Self {
            project,
            cfgs,
            path_trees,
            call_graphs,
            sequence_trees,
            annotations: RwLock::new(Vec::new()),
        })
    }
}
