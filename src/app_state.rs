use std::{sync::Arc, time::Duration};

use anyhow::Context;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::{
    ascii_server::{user_manager::UserManager, user_scheduler::UserScheduler},
    framebuffer::FrameBuffer,
    http_server::websocket::start_websocket_compressor_loop,
    proto::WebSocketMessage,
    statistics::Statistics,
};

pub struct AppState {
    pub user_manager: UserManager,
    pub user_scheduler: Arc<UserScheduler>,
    pub framebuffer: RwLock<FrameBuffer>,
    #[allow(unused)] // We probably need it later on (e.g. Prometheus metrics)
    pub statistics: Arc<Statistics>,

    pub ws_message_tx: mpsc::Sender<WebSocketMessage>,
    // TODO: Can we avoid cloning the [`Vec`] for every websocket connection?
    // Maybe have an Arc here?
    // See https://www.reddit.com/r/rust/comments/ms8yjz/how_to_send_a_slice_through_a_channel_confused/
    pub compressed_ws_message_rx: broadcast::Receiver<Vec<u8>>,
}

impl AppState {
    pub async fn new(slot_duration: Duration, width: u16, height: u16) -> anyhow::Result<Self> {
        // This only buffers between the server and the compression loop
        // There is a separate broadcast channel between the compression loop and individual websockets
        let (ws_message_tx, ws_message_rx) = mpsc::channel(32);
        let compressed_ws_message_rx = start_websocket_compressor_loop(ws_message_rx).await;

        let framebuffer = RwLock::new(FrameBuffer::new(width, height));
        let user_manager = UserManager::new_from_save_file()
            .await
            .context("Failed to create user manager")?;
        let user_scheduler = Arc::new(UserScheduler::new(ws_message_tx.clone(), slot_duration));
        let statistics = Arc::new(Statistics::new(
            ws_message_tx.clone(),
            user_scheduler.clone(),
        ));

        // Start user scheduler
        let user_scheduler_clone = user_scheduler.clone();
        tokio::spawn(async move {
            user_scheduler_clone.run().await?;
            anyhow::Ok(())
        });

        // Start statistics reporting
        let statistics_clone = statistics.clone();
        tokio::spawn(async move {
            statistics_clone.run().await?;
            anyhow::Ok(())
        });

        Ok(Self {
            user_manager,
            user_scheduler,
            framebuffer,
            statistics,
            ws_message_tx,
            compressed_ws_message_rx,
        })
    }
}
