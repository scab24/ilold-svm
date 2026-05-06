use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScenarioAction {
    New { name: String },
    List,
    Switch { name: String },
    Fork {
        name: String,
        #[serde(default)]
        at_step: Option<usize>,
    },
    Delete { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioInfo {
    pub name: String,
    pub active: bool,
    pub step_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScenarioEvent {
    Created { name: String },
    Switched { from: String, to: String },
    Deleted { name: String },
    Forked { from: String, to: String, at_step: usize },
    Reloaded { active: String },
}

pub fn validate_scenario_name(name: &str) -> Result<(), String> {
    const ERR: &str = "Invalid scenario name: must match ^[a-z][a-z0-9_-]{0,31}$";
    if name.is_empty() || name.len() > 32 {
        return Err(ERR.to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().ok_or_else(|| ERR.to_string())?;
    if !first.is_ascii_lowercase() {
        return Err(ERR.to_string());
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
            return Err(ERR.to_string());
        }
    }
    Ok(())
}
