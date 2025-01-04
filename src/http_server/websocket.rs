use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket},
    State,
};
use prost::Message as _;
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, trace, warn};

use crate::app_state::AppState;

pub async fn handle_websocket(mut ws: WebSocket, state: State<Arc<AppState>>) {
    info!("Websocket connected");

    let mut rx = state.web_socket_message_rx.resubscribe();

    loop {
        let web_socket_message = rx.recv().await;
        let web_socket_message = match web_socket_message {
            Ok(web_socket_message) => web_socket_message,
            Err(RecvError::Closed) => {
                // Server is shutting down
                break;
            }
            Err(RecvError::Lagged(lag)) => {
                warn!(lag, "The websocket loop has too much lag. Did the client fall behind? Maybe the browser crashed? It's better to miss some messages instead of slamming the websocket closed, continuing");
                continue;
            }
        };

        let bytes = web_socket_message.encode_to_vec();

        trace!(bytes = bytes.len(), "Sending websocket message");

        if ws.send(Message::Binary(bytes.into())).await.is_err() {
            break;
        }
    }

    info!("Websocket closed");
}
