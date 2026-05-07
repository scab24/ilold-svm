use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use ilold_core::classify::entry_points::AccessLevel;
use ilold_core::exploration::commands::{CanvasPatch, ScenarioEvent};

use crate::state::AppState;
use super::search::{self, SearchQuery};

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "search")]
    Search(SearchQuery),
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "search_result")]
    SearchResult(search::SearchResult),
    #[serde(rename = "search_complete")]
    SearchComplete { total: usize },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "session_add_node")]
    SessionAddNode { scenario: String, function: String, access: AccessLevel, step_index: usize },
    #[serde(rename = "session_remove_node")]
    SessionRemoveNode { scenario: String },
    #[serde(rename = "session_clear")]
    SessionClear { scenario: String },
    #[serde(rename = "session_highlight")]
    SessionHighlight { scenario: String, function: String },
    #[serde(rename = "scenario_created")]
    ScenarioCreated { name: String },
    #[serde(rename = "scenario_switched")]
    ScenarioSwitched { from: String, to: String },
    #[serde(rename = "scenario_deleted")]
    ScenarioDeleted { name: String },
    #[serde(rename = "scenario_forked")]
    ScenarioForked { from: String, to: String, at_step: usize },
    #[serde(rename = "scenario_store_reloaded")]
    ScenarioStoreReloaded { active: String },
    #[serde(rename = "solana_users_changed")]
    SolanaUsersChanged { scenario: String },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

fn server_message_from_patch(patch: CanvasPatch) -> ServerMessage {
    match patch {
        CanvasPatch::AddNode { scenario, function, access, step_index } => {
            ServerMessage::SessionAddNode { scenario, function, access, step_index }
        }
        CanvasPatch::RemoveLastNode { scenario } => ServerMessage::SessionRemoveNode { scenario },
        CanvasPatch::ClearAll { scenario } => ServerMessage::SessionClear { scenario },
        CanvasPatch::Highlight { scenario, function } => {
            ServerMessage::SessionHighlight { scenario, function }
        }
        CanvasPatch::ScenarioEvent(evt) => match evt {
            ScenarioEvent::Created { name } => ServerMessage::ScenarioCreated { name },
            ScenarioEvent::Switched { from, to } => ServerMessage::ScenarioSwitched { from, to },
            ScenarioEvent::Deleted { name } => ServerMessage::ScenarioDeleted { name },
            ScenarioEvent::Forked { from, to, at_step } => {
                ServerMessage::ScenarioForked { from, to, at_step }
            }
            ScenarioEvent::Reloaded { active } => {
                ServerMessage::ScenarioStoreReloaded { active }
            }
        },
        CanvasPatch::SolanaUsersChanged { scenario } => {
            ServerMessage::SolanaUsersChanged { scenario }
        }
    }
}

async fn send_json(socket: &mut WebSocket, msg: &ServerMessage) -> bool {
    let json = serde_json::to_string(msg).unwrap();
    socket.send(Message::Text(json.into())).await.is_ok()
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.session_tx.subscribe();

    loop {
        tokio::select! {
            client_msg = socket.recv() => {
                let msg = match client_msg {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                };

                let parsed: ClientMessage = match serde_json::from_str(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        let err = ServerMessage::Error { message: format!("Invalid message: {e}") };
                        if !send_json(&mut socket, &err).await { break; }
                        continue;
                    }
                };

                match parsed {
                    ClientMessage::Search(query) => {
                        let results = match state.solidity() {
                            Some(s) => search::search_paths(s, &query),
                            None => Vec::new(),
                        };
                        let total = results.len();

                        for result in results {
                            let msg = ServerMessage::SearchResult(result);
                            if !send_json(&mut socket, &msg).await { break; }
                        }

                        let complete = ServerMessage::SearchComplete { total };
                        if !send_json(&mut socket, &complete).await { break; }
                    }
                }
            }

            patch = rx.recv() => {
                match patch {
                    Ok(p) => {
                        let msg = server_message_from_patch(p);
                        if !send_json(&mut socket, &msg).await { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}
