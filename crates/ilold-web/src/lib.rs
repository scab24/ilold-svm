pub mod api;
pub mod state;
pub mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;

use state::AppState;

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/project", get(api::project::get_project))
        .route("/api/project/map", get(api::project::get_project_map))
        .route("/api/contract/{name}", get(api::contract::get_contract))
        .route("/api/contract/{name}/callgraph", get(api::contract::get_callgraph))
        .route("/api/contract/{name}/{func}/cfg", get(api::contract::get_cfg))
        .route("/api/contract/{name}/{func}/paths", get(api::contract::get_paths))
        .route("/api/contract/{name}/sequences", get(api::contract::get_sequences))
        .route("/api/contract/{name}/analysis", get(api::contract::get_sequence_analysis))
        .route("/api/contract/{name}/suggestions", get(api::contract::get_search_suggestions))
        .route("/api/annotations", get(api::annotations::list_annotations))
        .route("/api/annotations", post(api::annotations::create_annotation))
        .route("/api/annotations/{id}", put(api::annotations::update_annotation))
        .route("/api/annotations/{id}", delete(api::annotations::delete_annotation))
        .route("/api/cmd", post(api::session::handle_command))
        .route("/api/session/step/{index}/narrative", get(api::session::get_step_detail))
        .route("/api/session/state", get(api::session::get_state_detail))
        .route("/api/session/sequence", get(api::session::get_sequence_detail))
        .route("/api/session/function/{contract}/{func}", get(api::session::get_function_detail))
        .route("/api/session/trace/{contract}/{func}", get(api::session::get_flow_trace))
        .route("/ws", get(ws::handler::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve(paths: Vec<PathBuf>, port: u16, max_seq_depth: usize) -> anyhow::Result<()> {
    println!("Analyzing {} file(s)...", paths.len());
    let state = Arc::new(AppState::from_paths(&paths, max_seq_depth)?);
    println!(
        "Ready: {} contracts, {} functions analyzed\n",
        state.project.contracts.len(),
        state.cfgs.len(),
    );

    let app = build_router(state);
    let addr = format!("0.0.0.0:{port}");
    println!("Server running at http://localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn start_server(
    paths: Vec<PathBuf>,
    port: u16,
    max_seq_depth: usize,
) -> anyhow::Result<(Arc<AppState>, u16)> {
    let state = Arc::new(AppState::from_paths(&paths, max_seq_depth)?);
    let app = build_router(state.clone());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    let actual_port = listener.local_addr()?.port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((state, actual_port))
}
