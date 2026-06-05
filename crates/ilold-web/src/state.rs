use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use tokio::sync::broadcast;

use ilold_session_core::exploration::canvas::CanvasPatch;
use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::model::SolanaProject;
use solana_address::Address;
use solana_keypair::Keypair;

use serde::{Deserialize, Serialize};

pub const DEFAULT_SCENARIO: &str = "main";

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
        let mut order = file.order;
        order.retain(|n| file.scenarios.contains_key(n));
        for name in file.scenarios.keys() {
            if !order.contains(name) {
                order.push(name.clone());
            }
        }
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

pub type KeypairBundle = HashMap<String, HashMap<String, Keypair>>;

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

pub struct SolanaState {
    pub project: SolanaProject,
    pub program_artifacts: Vec<(Address, Vec<u8>)>,
    pub vms: RwLock<HashMap<String, VmHost>>,
    pub users: RwLock<HashMap<String, HashMap<String, Keypair>>>,
    pub step_snapshots: RwLock<HashMap<String, Vec<ilold_solana_core::execute::StateSnapshot>>>,
}

pub enum Backend {
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
    pub fn solana(&self) -> Option<&SolanaState> {
        let Backend::Solana(s) = &self.backend;
        Some(s)
    }
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
}
