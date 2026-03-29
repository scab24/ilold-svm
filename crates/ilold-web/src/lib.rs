pub mod api;
pub mod state;
pub mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;

use state::AppState;

pub async fn serve(paths: Vec<PathBuf>, port: u16, max_seq_depth: usize) -> anyhow::Result<()> {
    println!("Analyzing {} file(s)...", paths.len());
    let state = Arc::new(AppState::from_paths(&paths, max_seq_depth)?);
    println!(
        "Ready: {} contracts, {} functions analyzed\n",
        state.project.contracts.len(),
        state.cfgs.len(),
    );

    let app = Router::new()
        .route("/api/project", get(api::project::get_project))
        .route("/api/project/map", get(api::project::get_project_map))
        .route("/api/contract/{name}", get(api::contract::get_contract))
        .route("/api/contract/{name}/callgraph", get(api::contract::get_callgraph))
        .route("/api/contract/{name}/{func}/cfg", get(api::contract::get_cfg))
        .route("/api/contract/{name}/{func}/paths", get(api::contract::get_paths))
        .route("/api/contract/{name}/sequences", get(api::contract::get_sequences))
        .route("/api/contract/{name}/suggestions", get(api::contract::get_search_suggestions))
        .route("/api/annotations", get(api::annotations::list_annotations))
        .route("/api/annotations", post(api::annotations::create_annotation))
        .route("/api/annotations/{id}", put(api::annotations::update_annotation))
        .route("/api/annotations/{id}", delete(api::annotations::delete_annotation))
        .route("/ws", get(ws::handler::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    println!("Server running at http://localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
