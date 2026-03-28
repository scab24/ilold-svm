use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ProjectSummary {
    pub files: usize,
    pub contracts: Vec<ContractSummary>,
}

#[derive(Serialize)]
pub struct ContractSummary {
    pub name: String,
    pub kind: String,
    pub functions: usize,
    pub state_vars: usize,
    pub inherits: Vec<String>,
}

pub async fn get_project(State(state): State<Arc<AppState>>) -> Json<ProjectSummary> {
    let contracts = state
        .project
        .contracts
        .iter()
        .map(|c| ContractSummary {
            name: c.name.clone(),
            kind: format!("{:?}", c.kind),
            functions: c.functions.len(),
            state_vars: c.state_vars.len(),
            inherits: c.inherits.clone(),
        })
        .collect();

    Json(ProjectSummary {
        files: state.project.source_files.len(),
        contracts,
    })
}
