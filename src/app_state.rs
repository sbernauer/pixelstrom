use tokio::sync::{broadcast, mpsc, RwLock};

use crate::{framebuffer::FrameBuffer, proto::WebSocketMessage};

pub struct AppState {
    pub framebuffer: RwLock<FrameBuffer>,

    pub ws_message_tx: mpsc::Sender<WebSocketMessage>,
    // TODO: Can we avoid cloning the [`Vec`] for every websocket connection?
    // Maybe have an Arc here?
    // See https://www.reddit.com/r/rust/comments/ms8yjz/how_to_send_a_slice_through_a_channel_confused/
    pub compressed_ws_message_tx: broadcast::Receiver<Vec<u8>>,
}

impl AppState {
    pub fn new(
        width: u16,
        height: u16,
        ws_message_tx: mpsc::Sender<WebSocketMessage>,
        compressed_ws_message_tx: broadcast::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            framebuffer: RwLock::new(FrameBuffer::new(width, height)),
            ws_message_tx,
            compressed_ws_message_tx,
        }
    }
}
