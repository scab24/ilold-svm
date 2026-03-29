use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

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
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            _ => continue,
        };

        let client_msg: ClientMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(e) => {
                let err = ServerMessage::Error { message: format!("Invalid message: {e}") };
                let _ = socket.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                continue;
            }
        };

        match client_msg {
            ClientMessage::Search(query) => {
                let results = search::search_paths(&state, &query);
                let total = results.len();

                for result in results {
                    let msg = ServerMessage::SearchResult(result);
                    let json = serde_json::to_string(&msg).unwrap();
                    if socket.send(Message::Text(json.into())).await.is_err() {
                        return;
                    }
                }

                let complete = ServerMessage::SearchComplete { total };
                let json = serde_json::to_string(&complete).unwrap();
                let _ = socket.send(Message::Text(json.into())).await;
            }
        }
    }
}
