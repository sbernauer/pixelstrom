use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    RwLock,
};

use crate::{framebuffer::FrameBuffer, proto::WebSocketMessage};

#[derive(Debug)]
pub struct AppState {
    pub framebuffer: RwLock<FrameBuffer>,

    pub web_socket_message_tx: Sender<WebSocketMessage>,
    pub web_socket_message_rx: Receiver<WebSocketMessage>,
}

impl AppState {
    pub fn new(width: u16, height: u16) -> Self {
        let (web_socket_message_tx, web_socket_message_rx) = broadcast::channel(
            // Please note that this number is a trade-off:
            // To small capacity can cause websockets to fall behind and miss messages (we will log warnings in this case)
            // To high capacity can cause very high memory usage in case websocket clients fall behind
            1024,
        );

        Self {
            framebuffer: RwLock::new(FrameBuffer::new(width, height)),
            web_socket_message_tx,
            web_socket_message_rx,
        }
    }
}
