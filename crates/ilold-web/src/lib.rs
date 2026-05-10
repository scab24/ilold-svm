pub mod api;
pub mod state;
pub mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;
use ilold_solana_core::ingest::DetectedProject;
use ilold_solana_core::model::{ProgramDef, SolanaProject};
use tower_http::cors::CorsLayer;

use state::AppState;

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/project", get(api::project::get_project))
        .route("/api/project/map", get(api::project::get_project_map))
        .route("/api/program/{name}/view", get(api::project::get_program_view))
        .route("/api/program/{name}/overlay", get(api::project::get_program_overlay))
        .route("/api/contract/{name}", get(api::contract::get_contract))
        .route("/api/contract/{name}/callgraph", get(api::contract::get_callgraph))
        .route("/api/contract/{name}/{func}/cfg", get(api::contract::get_cfg))
        .route("/api/contract/{name}/{func}/paths", get(api::contract::get_paths))
        .route("/api/contract/{name}/{func}/source", get(api::contract::get_function_source))
        .route("/api/contract/{name}/sequences", get(api::contract::get_sequences))
        .route("/api/contract/{name}/analysis", get(api::contract::get_sequence_analysis))
        .route("/api/contract/{name}/suggestions", get(api::contract::get_search_suggestions))
        .route("/api/annotations", get(api::annotations::list_annotations))
        .route("/api/annotations", post(api::annotations::create_annotation))
        .route("/api/annotations/{id}", put(api::annotations::update_annotation))
        .route("/api/annotations/{id}", delete(api::annotations::delete_annotation))
        .route("/api/cmd", post(api::session::handle_command))
        .route("/api/session/step/{index}/narrative", get(api::session::get_step_detail))
        .route("/api/session/step/{index}/trace", get(api::session::get_session_step_trace))
        .route("/api/session/timeline/{variable}", get(api::session::get_variable_timeline_handler))
        .route("/api/session/state", get(api::session::get_state_detail))
        .route("/api/session/sequence", get(api::session::get_sequence_detail))
        .route("/api/session/function/{contract}/{func}", get(api::session::get_function_detail))
        .route("/api/session/trace/{contract}/{func}", get(api::session::get_flow_trace))
        .route("/api/session/slice/{func}/{variable}", get(api::session::get_function_slice))
        .route("/api/scenarios", get(api::session::get_scenarios))
        .route("/api/scenarios/all", get(api::session::get_all_scenarios))
        .route("/ws", get(ws::handler::ws_handler))
        .route("/ws/pty", get(ws::pty::ws_pty_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve(paths: Vec<PathBuf>, port: u16, max_seq_depth: usize) -> anyhow::Result<()> {
    println!("Analyzing {} file(s)...", paths.len());
    let project_root = paths.first().map(|p| p.parent().unwrap_or(p).to_path_buf()).unwrap_or_default();
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    let actual_port = listener.local_addr()?.port();
    let state = Arc::new(AppState::from_paths(&paths, max_seq_depth, actual_port, project_root)?);
    if let Some(s) = state.solidity() {
        println!(
            "Ready: {} contracts, {} functions analyzed\n",
            s.project.contracts.len(),
            s.cfgs.len(),
        );
    }

    let app = build_router(state);
    println!("Server running at http://localhost:{actual_port}");
    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn start_server(
    paths: Vec<PathBuf>,
    port: u16,
    max_seq_depth: usize,
) -> anyhow::Result<(Arc<AppState>, u16)> {
    let project_root = paths.first().map(|p| p.parent().unwrap_or(p).to_path_buf()).unwrap_or_default();
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    let actual_port = listener.local_addr()?.port();
    let state = Arc::new(AppState::from_paths(&paths, max_seq_depth, actual_port, project_root)?);
    let app = build_router(state.clone());

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((state, actual_port))
}

pub async fn serve_solana(detected: DetectedProject, port: u16) -> anyhow::Result<()> {
    println!("Analyzing {} IDL(s)...", detected.idl_paths.len());
    let project = build_solana_project(&detected)?;
    let artifacts = load_program_artifacts(&detected, &project);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    let actual_port = listener.local_addr()?.port();
    let state = Arc::new(AppState::from_solana(
        project,
        artifacts,
        actual_port,
        detected.root.clone(),
    )?);
    if let Some(s) = state.solana() {
        println!(
            "Ready: {} program(s) analyzed, {} .so loaded\n",
            s.project.programs.len(),
            s.program_artifacts.len()
        );
    }
    let app = build_router(state);
    println!("Server running at http://localhost:{actual_port}");
    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn start_solana_server(
    detected: DetectedProject,
    port: u16,
) -> anyhow::Result<(Arc<AppState>, u16)> {
    let project = build_solana_project(&detected)?;
    let artifacts = load_program_artifacts(&detected, &project);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    let actual_port = listener.local_addr()?.port();
    let state = Arc::new(AppState::from_solana(
        project,
        artifacts,
        actual_port,
        detected.root.clone(),
    )?);
    let app = build_router(state.clone());

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((state, actual_port))
}

fn build_solana_project(detected: &DetectedProject) -> anyhow::Result<SolanaProject> {
    let mut programs = Vec::with_capacity(detected.idl_paths.len());
    for idl_path in &detected.idl_paths {
        let json = std::fs::read_to_string(idl_path)?;
        let idl = ilold_solana_core::idl::parse_idl(&json)?;
        let program = ProgramDef::from_idl(idl)?;
        programs.push(program);
    }
    Ok(SolanaProject::new(programs))
}

fn load_program_artifacts(
    detected: &DetectedProject,
    project: &SolanaProject,
) -> Vec<(solana_address::Address, Vec<u8>)> {
    let mut out = Vec::new();
    for program in &project.programs {
        for so_path in &detected.so_paths {
            let stem = so_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if stem == program.name {
                if let Ok(bytes) = std::fs::read(so_path) {
                    out.push((program.program_id, bytes));
                }
                break;
            }
        }
    }
    out
}
