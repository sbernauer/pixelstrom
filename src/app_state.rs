use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    RwLock,
};

use crate::{framebuffer::FrameBuffer, proto::WebSocketMessage};

#[derive(Debug)]
pub struct AppState {
    pub framebuffer: RwLock<FrameBuffer>,
    pub web_socket_message_rx: Receiver<WebSocketMessage>,
}

impl AppState {
    pub fn new(width: u32, height: u32) -> (Self, Sender<WebSocketMessage>) {
        let (web_socket_message_tx, web_socket_message_rx) = broadcast::channel(16);
        (
            Self {
                framebuffer: RwLock::new(FrameBuffer::new(width, height)),
                web_socket_message_rx,
            },
            web_socket_message_tx,
        )
    }
}
