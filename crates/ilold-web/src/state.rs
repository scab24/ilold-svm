use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use axum::http::StatusCode;
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
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::model::SolanaProject;
use solana_address::Address;
use solana_keypair::Keypair;

use serde::{Deserialize, Serialize};

/// The default scenario name, auto-created for every fresh session.
pub const DEFAULT_SCENARIO: &str = "main";

/// Holds all scenarios for a contract. One is "active" at any time; commands
/// without explicit scenario targeting operate on it. Insertion order is
/// preserved via `order` for deterministic `names()` output.
pub struct ScenarioStore {
    pub version: u32,
    pub contract: String,
    active: String,
    sessions: HashMap<String, ExplorationSession>,
    order: Vec<String>,
}

impl ScenarioStore {
    pub fn new_for_contract(contract: impl Into<String>) -> Self {
        let contract = contract.into();
        let mut sessions = HashMap::new();
        sessions.insert(
            DEFAULT_SCENARIO.to_string(),
            ExplorationSession::new(&contract, "ilold"),
        );
        Self {
            version: 2,
            contract,
            active: DEFAULT_SCENARIO.to_string(),
            sessions,
            order: vec![DEFAULT_SCENARIO.to_string()],
        }
    }

    pub fn active(&self) -> &str {
        &self.active
    }

    pub fn active_session(&self) -> &ExplorationSession {
        self.sessions
            .get(&self.active)
            .expect("active scenario always present")
    }

    pub fn active_session_mut(&mut self) -> &mut ExplorationSession {
        self.sessions
            .get_mut(&self.active)
            .expect("active scenario always present")
    }

    pub fn get(&self, name: &str) -> Option<&ExplorationSession> {
        self.sessions.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ExplorationSession> {
        self.sessions.get_mut(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.sessions.contains_key(name)
    }

    pub fn names(&self) -> &[String] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn insert(&mut self, name: impl Into<String>, session: ExplorationSession) {
        let name = name.into();
        if !self.sessions.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.sessions.insert(name, session);
    }

    pub fn remove(&mut self, name: &str) -> Option<ExplorationSession> {
        let removed = self.sessions.remove(name)?;
        self.order.retain(|n| n != name);
        Some(removed)
    }

    pub fn set_active(&mut self, name: impl Into<String>) -> Result<(), String> {
        let name = name.into();
        if !self.sessions.contains_key(&name) {
            return Err(format!("Scenario '{name}' does not exist"));
        }
        self.active = name;
        Ok(())
    }

    /// Serialize the entire store as v2 JSON (`{ version: 2, contract, active,
    /// scenarios, order, [keypairs_present, keypairs] }`). When `opts.keypairs`
    /// is `Some`, the file embeds the per-scenario user keypairs as 64-byte
    /// arrays so a future load can rehydrate the same identities (PDAs and
    /// signatures match across save/load). The boolean header
    /// `keypairs_present` lets a reader detect a bundle with secrets without
    /// parsing the body — see SDD-03 design.md threat-model section.
    pub fn save_to_json(&self, opts: SaveOpts<'_>) -> Result<String, String> {
        let (keypairs_present, keypairs) = match opts.keypairs {
            Some(map) => {
                let serialised: HashMap<String, HashMap<String, Vec<u8>>> = map
                    .iter()
                    .map(|(scn, users)| {
                        let inner: HashMap<String, Vec<u8>> = users
                            .iter()
                            .map(|(name, kp)| (name.clone(), kp.to_bytes().to_vec()))
                            .collect();
                        (scn.clone(), inner)
                    })
                    .collect();
                (true, Some(serialised))
            }
            None => (false, None),
        };
        let file = ScenarioStoreFile {
            version: 2,
            contract: self.contract.clone(),
            active: self.active.clone(),
            scenarios: self.sessions.clone(),
            order: self.order.clone(),
            keypairs_present,
            keypairs,
        };
        serde_json::to_string_pretty(&file).map_err(|e| format!("Serialize failed: {e}"))
    }

    /// Parse a save file. Tries v2 (`ScenarioStoreFile`) first; on failure
    /// falls back to v1 (bare `ExplorationSession`) and wraps it as a single
    /// `main` scenario. Any structural anomaly (active not in scenarios,
    /// empty order) is repaired so the returned store is always valid.
    pub fn load_from_json(json: &str) -> Result<(Self, Option<KeypairBundle>), String> {
        match serde_json::from_str::<ScenarioStoreFile>(json) {
            Ok(file) => {
                let raw_kps = file.keypairs.clone();
                let store = Self::from_file(file)?;
                let bundle = match raw_kps {
                    Some(map) => Some(decode_keypair_bundle(map)?),
                    None => None,
                };
                Ok((store, bundle))
            }
            Err(_) => match serde_json::from_str::<ExplorationSession>(json) {
                Ok(legacy) => {
                    let contract = legacy.contract.clone();
                    let mut sessions = HashMap::new();
                    sessions.insert(DEFAULT_SCENARIO.to_string(), legacy);
                    Ok((
                        Self {
                            version: 2,
                            contract,
                            active: DEFAULT_SCENARIO.to_string(),
                            sessions,
                            order: vec![DEFAULT_SCENARIO.to_string()],
                        },
                        None,
                    ))
                }
                Err(e) => Err(format!("Deserialize failed: {e}")),
            },
        }
    }

    fn from_file(file: ScenarioStoreFile) -> Result<Self, String> {
        if file.scenarios.is_empty() {
            return Err("Save file has no scenarios".into());
        }
        // Repair `order`: if missing names or empty, rebuild from scenarios.
        let mut order = file.order;
        order.retain(|n| file.scenarios.contains_key(n));
        for name in file.scenarios.keys() {
            if !order.contains(name) {
                order.push(name.clone());
            }
        }
        // Repair `active`: fall back to first ordered name if the recorded
        // active was deleted out-of-band before the save.
        let active = if file.scenarios.contains_key(&file.active) {
            file.active
        } else {
            order
                .first()
                .cloned()
                .ok_or_else(|| "Save file has no usable active scenario".to_string())?
        };
        Ok(Self {
            version: 2,
            contract: file.contract,
            active,
            sessions: file.scenarios,
            order,
        })
    }
}

/// On-disk wire format for `ScenarioStore`. Decoupled from the in-memory
/// type to keep private fields private and allow the wire format to evolve
/// independently. v1 saves are bare `ExplorationSession` JSON — they're
/// detected by the failed parse + retry in `ScenarioStore::load_from_json`.
///
/// SDD-03 added the optional `keypairs_present` / `keypairs` pair, both
/// `#[serde(default)]` so older v2 saves keep loading.
#[derive(Serialize, Deserialize)]
struct ScenarioStoreFile {
    version: u32,
    contract: String,
    active: String,
    scenarios: HashMap<String, ExplorationSession>,
    order: Vec<String>,
    #[serde(default)]
    keypairs_present: bool,
    #[serde(default)]
    keypairs: Option<HashMap<String, HashMap<String, Vec<u8>>>>,
}

/// Bundle of per-scenario, per-user-name keypairs surfaced by
/// `ScenarioStore::load_from_json`. Solana's LoadSession dispatcher uses it
/// to rehydrate `solana.users` before the replay loop runs.
pub type KeypairBundle = HashMap<String, HashMap<String, Keypair>>;

/// Options for `ScenarioStore::save_to_json`. Today only `keypairs` is
/// configurable; future work (encrypted bundle) will extend this struct
/// without breaking call sites.
pub struct SaveOpts<'a> {
    pub keypairs: Option<&'a HashMap<String, HashMap<String, Keypair>>>,
}

impl<'a> SaveOpts<'a> {
    pub fn none() -> Self {
        Self { keypairs: None }
    }
}

fn decode_keypair_bundle(
    raw: HashMap<String, HashMap<String, Vec<u8>>>,
) -> Result<KeypairBundle, String> {
    let mut out: KeypairBundle = HashMap::new();
    for (scn, users) in raw {
        let mut inner: HashMap<String, Keypair> = HashMap::new();
        for (name, bytes) in users {
            if bytes.len() != 64 {
                return Err(format!(
                    "keypair for {scn}/{name} must be 64 bytes, got {}",
                    bytes.len()
                ));
            }
            let kp = Keypair::try_from(bytes.as_slice()).map_err(|_| {
                format!("invalid keypair bytes for {scn}/{name} (ed25519 decode failed)")
            })?;
            inner.insert(name, kp);
        }
        out.insert(scn, inner);
    }
    Ok(out)
}

pub struct SolidityState {
    pub project: Project,
    pub cfgs: HashMap<(String, String), CfgGraph>,
    pub path_trees: HashMap<(String, String), PathTree>,
    pub call_graphs: HashMap<String, CallGraph>,
    pub sequence_trees: HashMap<String, SequenceTree>,
    pub sequence_analyses: HashMap<String, SequenceAnalysis>,
    pub classifications: HashMap<String, Vec<(String, AccessLevel)>>,
}

pub struct SolanaState {
    pub project: SolanaProject,
    pub program_artifacts: Vec<(Address, Vec<u8>)>,
    pub vms: RwLock<HashMap<String, VmHost>>,
    pub users: RwLock<HashMap<String, HashMap<String, Keypair>>>,
    /// Per-scenario stack of pre-Call snapshots, indexed by step. Used by
    /// `Back` to rewind the VM state and by `Fork(at_step=N)` to seed the
    /// new scenario's VM with the N-th snapshot. Cleared on `Clear` and
    /// truncated on `Back`. Empty after `LoadSession` (replay rebuilds it).
    pub step_snapshots: RwLock<HashMap<String, Vec<ilold_solana_core::execute::StateSnapshot>>>,
}

pub enum Backend {
    Solidity(SolidityState),
    Solana(SolanaState),
}

pub struct AppState {
    pub backend: Backend,
    pub annotations: RwLock<Vec<Annotation>>,
    pub scenarios: RwLock<ScenarioStore>,
    pub session_tx: broadcast::Sender<CanvasPatch>,
    pub port: u16,
    pub project_root: PathBuf,
}

impl AppState {
    pub fn solidity(&self) -> Option<&SolidityState> {
        match &self.backend {
            Backend::Solidity(s) => Some(s),
            Backend::Solana(_) => None,
        }
    }

    pub fn solana(&self) -> Option<&SolanaState> {
        match &self.backend {
            Backend::Solana(s) => Some(s),
            Backend::Solidity(_) => None,
        }
    }
}

impl AppState {
    pub fn unwrap_solidity(&self) -> &SolidityState {
        self.solidity().expect("Solidity backend required")
    }
}

pub fn require_solidity(state: &AppState) -> Result<&SolidityState, StatusCode> {
    state.solidity().ok_or(StatusCode::BAD_REQUEST)
}

pub fn require_solidity_msg(state: &AppState) -> Result<&SolidityState, (StatusCode, String)> {
    state
        .solidity()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "endpoint is Solidity-only".to_string()))
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
    pub fn from_solana(
        project: SolanaProject,
        program_artifacts: Vec<(Address, Vec<u8>)>,
        port: u16,
        project_root: PathBuf,
    ) -> anyhow::Result<Self> {
        let (session_tx, _) = broadcast::channel(64);
        let default_program = project
            .programs
            .first()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let mut vms = HashMap::new();
        let main_vm = VmHost::boot(program_artifacts.clone())
            .map_err(|e| anyhow::anyhow!("boot main VM: {e:?}"))?;
        vms.insert(DEFAULT_SCENARIO.to_string(), main_vm);

        let mut users: HashMap<String, HashMap<String, Keypair>> = HashMap::new();
        users.insert(DEFAULT_SCENARIO.to_string(), HashMap::new());

        let mut step_snapshots: HashMap<String, Vec<ilold_solana_core::execute::StateSnapshot>> =
            HashMap::new();
        step_snapshots.insert(DEFAULT_SCENARIO.to_string(), Vec::new());

        Ok(Self {
            backend: Backend::Solana(SolanaState {
                project,
                program_artifacts,
                vms: RwLock::new(vms),
                users: RwLock::new(users),
                step_snapshots: RwLock::new(step_snapshots),
            }),
            annotations: RwLock::new(Vec::new()),
            scenarios: RwLock::new(ScenarioStore::new_for_contract(default_program)),
            session_tx,
            port,
            project_root,
        })
    }

    pub fn from_paths(paths: &[PathBuf], max_seq_depth: usize, port: u16, project_root: PathBuf) -> anyhow::Result<Self> {
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

        let default_contract = project.contracts.iter()
            .find(|c| !c.name.is_empty())
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            backend: Backend::Solidity(SolidityState {
                project,
                cfgs,
                path_trees,
                call_graphs,
                sequence_trees,
                sequence_analyses,
                classifications,
            }),
            annotations: RwLock::new(Vec::new()),
            scenarios: RwLock::new(ScenarioStore::new_for_contract(default_contract)),
            session_tx,
            port,
            project_root,
        })
    }
}
