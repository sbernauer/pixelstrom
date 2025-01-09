use std::sync::Arc;

use anyhow::Context;
use axum::extract::{
    ws::{Message, WebSocket},
    State,
};
use prost::Message as _;
use tokio::sync::{
    broadcast::{self, error::RecvError},
    mpsc,
};
use tracing::{error, info, instrument, trace, warn};
use zstd::DEFAULT_COMPRESSION_LEVEL;

use crate::{
    app_state::AppState,
    proto::{web_socket_message::Payload, WebSocketClosedBecauseOfLag, WebSocketMessage},
};

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
                warn!(
                    lag,
                    "The websocket loop has too much lag, closing connection"
                );

                let compressed_ws_message = match web_socket_closed_because_of_lag_message(lag) {
                    Ok(compressed_ws_message) => compressed_ws_message,
                    Err(err) => {
                        error!(
                            error = %err,
                            "Failed to compress websocket message"
                        );

                        break; // Close connection
                    }
                };

                if let Err(err) = ws.send(Message::binary(compressed_ws_message)).await {
                    error!(
                        error = &err as &dyn std::error::Error,
                        "Failed to send compressed websocket message to websocket, closing websocket anyway"
                    );
                }

                break; // Close connection in any case
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
            // As the compression can take a while we put it on the blocking threadpool
            // TODO: Use tracing to inspect actual runtime of the command
            let compressed_bytes =
                tokio::task::spawn_blocking(move || compress_message(&ws_message)).await;

            let (compressed_bytes, uncompressed_bytes_len) = match compressed_bytes {
                Ok(Ok(compressed_bytes)) => compressed_bytes,
                Ok(Err(err)) => {
                    error!(
                        error = %err,
                        "Failed to compress websocket message"
                    );
                    continue;
                }
                Err(err) => {
                    error!(
                        error = &err as &dyn std::error::Error,
                        "Failed to join task that compresses websocket message"
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

/// Return the compressed bytes as well as the number of uncompressed bytes
#[instrument(skip(ws_message))] // ws_message can be pretty big
fn compress_message(ws_message: &WebSocketMessage) -> anyhow::Result<(Vec<u8>, usize)> {
    let uncompressed_bytes = ws_message.encode_to_vec();
    let compressed_bytes = zstd::encode_all(uncompressed_bytes.as_slice(), ZSTD_COMPRESSION_LEVEL)
        .with_context(|| {
            format!(
                "Failed to compress bytes of websocket message with {} bytes using zstd compression",
                uncompressed_bytes.len()
            )
        })?;

    Ok((compressed_bytes, uncompressed_bytes.len()))
}

fn web_socket_closed_because_of_lag_message(lag: u64) -> anyhow::Result<Vec<u8>> {
    let ws_message = WebSocketMessage {
        payload: Some(Payload::WebSocketClosedBecauseOfLag(
            WebSocketClosedBecauseOfLag { lag },
        )),
    };

    Ok(compress_message(&ws_message)?.0)
}
