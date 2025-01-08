use std::sync::Arc;

use axum::extract::{
    ws::{Message, WebSocket},
    State,
};
use prost::Message as _;
use tokio::sync::{
    broadcast::{self, error::RecvError},
    mpsc,
};
use tracing::{error, info, trace, warn};
use zstd::DEFAULT_COMPRESSION_LEVEL;

use crate::{app_state::AppState, proto::WebSocketMessage};

const ZSTD_COMPRESSION_LEVEL: i32 = DEFAULT_COMPRESSION_LEVEL;

pub async fn handle_websocket(mut ws: WebSocket, state: State<Arc<AppState>>) {
    info!("Websocket connected");

    let mut rx = state.compressed_ws_message_tx.resubscribe();

    loop {
        let compressed_ws_message = rx.recv().await;
        let compressed_ws_message = match compressed_ws_message {
            Ok(compressed_ws_message) => compressed_ws_message,
            Err(RecvError::Closed) => {
                // Server is shutting down
                break;
            }
            Err(RecvError::Lagged(lag)) => {
                warn!(lag, "The websocket loop has too much lag. Did the client fall behind? Maybe the browser crashed? It's better to miss some messages instead of slamming the websocket closed, continuing");
                continue;
            }
        };

        if let Err(err) = ws.send(Message::binary(compressed_ws_message)).await {
            error!(
                error = &err as &dyn std::error::Error,
                "Failed to send compressed websocket message to websocket, closing websocket"
            );
            break;
        }
    }

    info!("Websocket closed");
}

pub async fn start_websocket_compressor_loop(
    mut ws_message_rx: mpsc::Receiver<WebSocketMessage>,
) -> broadcast::Receiver<Vec<u8>> {
    let (compressed_ws_message_tx, compressed_ws_message_rx) = broadcast::channel(
        // Please note that this number is a trade-off:
        // To small capacity can cause websockets to fall behind and miss messages (we will log warnings in this case)
        // To high capacity can cause very high memory usage in case websocket clients fall behind
        512,
    );

    tokio::spawn(async move {
        while let Some(ws_message) = ws_message_rx.recv().await {
            let compressed_bytes = tokio::task::spawn_blocking(move || {
                let uncompressed_bytes = ws_message.encode_to_vec();

                let compressed_bytes =
                    zstd::encode_all(uncompressed_bytes.as_slice(), ZSTD_COMPRESSION_LEVEL)?;

                Ok::<_, std::io::Error>((compressed_bytes, uncompressed_bytes.len()))
            })
            .await;

            let (compressed_bytes, uncompressed_bytes_len) = match compressed_bytes {
                Ok(Ok(compressed_bytes)) => compressed_bytes,
                Ok(Err(err)) => {
                    error!(
                        error = &err as &dyn std::error::Error,
                        "Failed to compress websocket message using zstd compression"
                    );
                    continue;
                }
                Err(err) => {
                    error!(
                        error = &err as &dyn std::error::Error,
                        "Failed to join task that compresses websocket message using zstd compression"
                    );
                    continue;
                }
            };

            trace!(
                compression_ratio = uncompressed_bytes_len / compressed_bytes.len(),
                compressed_bytes = compressed_bytes.len(),
                uncompressed_bytes = uncompressed_bytes_len,
                "Compressed websocket message"
            );

            if let Err(err) = compressed_ws_message_tx.send(compressed_bytes) {
                error!(
                    error = &err as &dyn std::error::Error,
                    "Failed to send compressed websocket message to channel"
                );
            }
        }
    });

    compressed_ws_message_rx
}
