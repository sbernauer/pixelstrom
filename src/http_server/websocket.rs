use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket},
    State,
};
use prost::Message as _;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info, trace, warn};
use zstd::DEFAULT_COMPRESSION_LEVEL;

use crate::app_state::AppState;

const ZSTD_COMPRESSION_LEVEL: i32 = DEFAULT_COMPRESSION_LEVEL;

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

        // TODO: Ideally we only compress the message once and not individually for every websocket!
        let uncompressed_bytes = web_socket_message.encode_to_vec();
        let compressed_bytes =
            match zstd::encode_all(uncompressed_bytes.as_slice(), ZSTD_COMPRESSION_LEVEL) {
                Ok(compressed) => compressed,
                Err(err) => {
                    error!(
                        %err,
                        "Failed to compress websocket message using zstd compression"
                    );
                    continue;
                }
            };

        trace!(
            compressed_bytes = compressed_bytes.len(),
            uncompressed_bytes = uncompressed_bytes.len(),
            compression_ratio = uncompressed_bytes.len() / compressed_bytes.len(),
            "Sending websocket message"
        );

        if ws
            .send(Message::Binary(compressed_bytes.into()))
            .await
            .is_err()
        {
            break;
        }
    }

    info!("Websocket closed");
}
