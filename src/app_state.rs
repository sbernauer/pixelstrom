use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    RwLock,
};

use crate::{framebuffer::FrameBuffer, ScreenSync};

#[derive(Debug)]
pub struct AppState {
    pub framebuffer: RwLock<FrameBuffer>,
    pub screen_sync_rx: Receiver<ScreenSync>,
}

impl AppState {
    pub fn new(width: u32, height: u32) -> (Self, Sender<ScreenSync>) {
        let (screen_sync_tx, screen_sync_rx) = broadcast::channel(16);
        (
            Self {
                framebuffer: RwLock::new(FrameBuffer::new(width, height)),
                screen_sync_rx,
            },
            screen_sync_tx,
        )
    }
}
