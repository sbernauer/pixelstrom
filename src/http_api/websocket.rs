use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket},
    State,
};
use prost::Message as _;
use tracing::info;

use crate::app_state::AppState;

pub async fn handle_websocket(mut ws: WebSocket, state: State<Arc<AppState>>) {
    info!("Websocket connected");

    let mut rx = state.screen_sync_rx.resubscribe();
    while let Ok(screen_sync) = rx.recv().await {
        let bytes = screen_sync.encode_to_vec();
        if ws.send(Message::Binary(bytes)).await.is_err() {
            break;
        }
    }

    info!("Websocket closed");
}
