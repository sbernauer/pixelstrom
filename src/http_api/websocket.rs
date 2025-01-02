use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket},
    State,
};
use prost::Message as _;
use tracing::{info, trace};

use crate::app_state::AppState;

pub async fn handle_websocket(mut ws: WebSocket, state: State<Arc<AppState>>) {
    info!("Websocket connected");

    let mut rx = state.web_socket_message_rx.resubscribe();
    while let Ok(web_socket_message) = rx.recv().await {
        let bytes = web_socket_message.encode_to_vec();

        trace!(bytes = bytes.len(), "Sending websocket message");

        if ws.send(Message::Binary(bytes.into())).await.is_err() {
            break;
        }
    }

    info!("Websocket closed");
}
