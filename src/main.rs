use std::{ops::Deref, sync::Arc, time::Duration};

use http_api::build_router;
use prost::bytes::BufMut;
use rand::Rng;
use tokio::{net::TcpListener, sync::broadcast::Sender, time::interval};
use tracing::info;

use crate::{
    app_state::AppState,
    proto::{web_socket_message::Payload, ClientPainting, WebSocketMessage},
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
    let web_socket_message_tx_for_loop = web_socket_message_tx.clone();
    tokio::spawn(async move {
        rainbow_loop(shared_state_for_loop, web_socket_message_tx_for_loop).await
    });
    tokio::spawn(
        async move { random_client_paints_loop(width, height, web_socket_message_tx).await },
    );

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

const SIZE: u32 = 200;
async fn random_client_paints_loop(
    width: u32,
    height: u32,
    web_socket_message_tx: Sender<WebSocketMessage>,
) {
    let mut interval = interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        let mut rng = rand::thread_rng();
        let color: u32 = rng.gen();

        let start_x = rng.gen_range(0..width.saturating_sub(SIZE));
        let start_y = rng.gen_range(0..height.saturating_sub(SIZE));
        let end_x = start_x + SIZE;
        let end_y = start_y + SIZE;

        let mut painted = Vec::new();
        for x in start_x..end_x {
            for y in start_y..end_y {
                painted.put_u16(x as u16);
                painted.put_u16(y as u16);
                painted.put_u32(color);
            }
        }

        let ws_message = WebSocketMessage {
            payload: Some(Payload::ClientPainting(ClientPainting {
                client: "Sebidooo".to_owned(),
                painted,
            })),
        };

        web_socket_message_tx
            .send(ws_message)
            .expect("Failed to send ClientPainting to channel");
    }
}
