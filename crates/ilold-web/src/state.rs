use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use tokio::sync::broadcast;

use ilold_core::callgraph::builder::build_call_graph;
use ilold_core::exploration::commands::CanvasPatch;
use ilold_core::callgraph::types::CallGraph;
use ilold_core::cfg::builder::CfgBuilder;
use ilold_core::cfg::types::CfgGraph;
use ilold_core::classify::entry_points::{classify_all, AccessLevel};
use ilold_core::exploration::session::ExplorationSession;
use ilold_core::model::project::Project;
use ilold_core::parse::solar_frontend::SolarParser;
use ilold_core::parse::ProjectParser;
use ilold_core::pathtree::config::PruningConfig;
use ilold_core::pathtree::types::PathTree;
use ilold_core::pathtree::walker::build_path_tree;
use ilold_core::sequence::analysis::{analyze_project, analyze_sequences, SequenceAnalysis};
use ilold_core::sequence::builder::build_sequence_tree;
use ilold_core::sequence::types::SequenceTree;

use serde::{Deserialize, Serialize};

pub struct AppState {
    pub project: Project,
    pub cfgs: HashMap<(String, String), CfgGraph>,
    pub path_trees: HashMap<(String, String), PathTree>,
    pub call_graphs: HashMap<String, CallGraph>,
    pub sequence_trees: HashMap<String, SequenceTree>,
    pub sequence_analyses: HashMap<String, SequenceAnalysis>,
    pub classifications: HashMap<String, Vec<(String, AccessLevel)>>,
    pub annotations: RwLock<Vec<Annotation>>,
    pub session: RwLock<Option<ExplorationSession>>,
    pub session_tx: broadcast::Sender<CanvasPatch>,
    pub port: u16,
    pub contract_path: PathBuf,
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
    pub fn from_paths(paths: &[PathBuf], max_seq_depth: usize, port: u16, contract_path: PathBuf) -> anyhow::Result<Self> {
        let parser = SolarParser;
        let mut project = parser.parse(paths)?;
        project.rebuild_index();

        let config = PruningConfig::default();
        let mut cfgs = HashMap::new();
        let mut path_trees = HashMap::new();
        let mut call_graphs = HashMap::new();
        let mut sequence_trees = HashMap::new();
        let mut sequence_analyses = HashMap::new();
        let mut classifications = HashMap::new();

        for contract in &project.contracts {
            let cg = build_call_graph(&project, contract);
            call_graphs.insert(contract.name.clone(), cg);

            let mut contract_path_trees = Vec::new();
            let combined_state_vars = project.inherited_state_vars(contract);

            for func in &contract.functions {
                let key = (contract.name.clone(), func.name.clone());

                if let Ok(cfg) = CfgBuilder::build_with_project(func, contract, Some(&project)) {
                    let pt = build_path_tree(
                        &cfg,
                        &contract.name,
                        &func.name,
                        &combined_state_vars,
                        &config,
                    );
                    contract_path_trees.push(pt.clone());
                    path_trees.insert(key.clone(), pt);
                    cfgs.insert(key, cfg);
                }
            }

            let st = build_sequence_tree(contract, &contract_path_trees, max_seq_depth);
            sequence_trees.insert(contract.name.clone(), st);

            let pt_map: HashMap<(String, String), PathTree> = contract_path_trees
                .iter()
                .map(|pt| ((pt.contract.clone(), pt.function.clone()), pt.clone()))
                .collect();
            let analysis = analyze_sequences(&pt_map, &contract.name);
            sequence_analyses.insert(contract.name.clone(), analysis);

            classifications.insert(contract.name.clone(), classify_all(contract));
        }

        // Compute transitive effects across contracts (inheritance-aware).
        analyze_project(&project, &mut sequence_analyses);

        let (session_tx, _) = broadcast::channel(64);

        Ok(Self {
            project,
            cfgs,
            path_trees,
            call_graphs,
            sequence_trees,
            sequence_analyses,
            classifications,
            annotations: RwLock::new(Vec::new()),
            session: RwLock::new(None),
            session_tx,
            port,
            contract_path,
        })
    }
}
