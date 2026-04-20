use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tracing::info;

use crate::web::{
    middleware::auth::AuthenticatedUser, state::AppState, ws_broadcaster::WsBroadcaster,
};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> Response {
    let broadcaster = state.ws_broadcaster.clone();
    let user_id = auth.user_id.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, broadcaster, user_id))
}

async fn handle_socket(mut socket: WebSocket, broadcaster: Arc<WsBroadcaster>, user_id: String) {
    info!(user_id = %user_id, "WebSocket connected");
    let mut rx = broadcaster.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
        }
    }

    info!(user_id = %user_id, "WebSocket disconnected");
}
