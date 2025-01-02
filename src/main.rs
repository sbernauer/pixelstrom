use std::{ops::Deref, sync::Arc, time::Duration};

use http_api::build_router;
use tokio::{net::TcpListener, sync::broadcast::Sender, time::interval};
use tracing::info;

use crate::{
    app_state::AppState,
    proto::{web_socket_message::Payload, WebSocketMessage},
};

pub mod app_state;
pub mod framebuffer;
pub mod http_api;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/pixelstrom.rs"));
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let width = 1920;
    let height = 1080;

    let (app_state, web_socket_message_tx) = AppState::new(width, height);
    let shared_state = Arc::new(app_state);

    let shared_state_for_loop = shared_state.clone();
    tokio::spawn(async move { rainbow_loop(shared_state_for_loop, web_socket_message_tx).await });

    let app = build_router(shared_state);
    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to 0.0.0.0:3000");

    info!("Starting server at http://localhost:3000");
    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

async fn rainbow_loop(
    shared_state: Arc<AppState>,
    web_socket_message_tx: Sender<WebSocketMessage>,
) {
    let mut interval = interval(Duration::from_millis(2000));
    loop {
        interval.tick().await;
        {
            let mut fb = shared_state.framebuffer.write().await;
            fb.fill_with_rainbow();
        }

        let fb = shared_state.framebuffer.read().await;
        let screen_sync: proto::ScreenSync = fb.deref().into();

        web_socket_message_tx
            .send(WebSocketMessage {
                payload: Some(Payload::ScreenSync(screen_sync)),
            })
            .expect("Failed to send ScreenSync to channel");
    }
}
