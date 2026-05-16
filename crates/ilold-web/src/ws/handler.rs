use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Serialize;
use tokio::sync::broadcast;

use ilold_session_core::exploration::access::AccessLevel;
use ilold_session_core::exploration::canvas::CanvasPatch;
use ilold_session_core::exploration::scenario::ScenarioEvent;

use crate::state::AppState;

#[derive(Serialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "session_add_node")]
    SessionAddNode {
        scenario: String,
        function: String,
        access: AccessLevel,
        step_index: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime: Option<ilold_session_core::exploration::canvas::RuntimeMeta>,
    },
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
    #[serde(rename = "session_overlay_update")]
    SessionOverlayUpdate {
        scenario: String,
        ix_name: String,
        calls_added: u32,
        failed_added: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        cu: Option<u64>,
        cpi_targets_added: Vec<String>,
    },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

fn server_message_from_patch(patch: CanvasPatch) -> ServerMessage {
    match patch {
        CanvasPatch::AddNode { scenario, function, access, step_index, runtime } => {
            ServerMessage::SessionAddNode { scenario, function, access, step_index, runtime }
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
        CanvasPatch::OverlayUpdate {
            scenario,
            ix_name,
            calls_added,
            failed_added,
            cu,
            cpi_targets_added,
        } => ServerMessage::SessionOverlayUpdate {
            scenario,
            ix_name,
            calls_added,
            failed_added,
            cu,
            cpi_targets_added,
        },
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
                match client_msg {
                    Some(Ok(Message::Text(_))) => continue,
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
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
