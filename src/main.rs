use std::{ops::Deref, sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::{get, get_service},
    Router,
};
use prost::Message as _;
use tokio::{sync::broadcast::Sender, time::interval};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use crate::app_state::AppState;

pub mod app_state;
pub mod framebuffer;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/pixelstrom.rs"));
}
use proto::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let width = 1920;
    let height = 1080;
    // let width = 50;
    // let height = 50;

    let (app_state, screen_sync_tx) = AppState::new(width, height);
    let shared_state = Arc::new(app_state);

    let shared_state_for_loop = shared_state.clone();
    tokio::spawn(async move { random_color_loop(shared_state_for_loop, screen_sync_tx).await });

    let app = Router::new()
        .route_service("/", ServeFile::new("static/index.html"))
        .route(
            "/ws",
            get(
                |ws: WebSocketUpgrade, state: State<Arc<AppState>>| async move {
                    ws.on_upgrade(move |socket| handle_websocket(socket, state))
                },
            ),
        )
        .nest_service("/static", get_service(ServeDir::new("./static")))
        .with_state(shared_state);

    // Serve the app
    println!("Server running at http://localhost:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_websocket(mut ws: WebSocket, state: State<Arc<AppState>>) {
    info!("Websocket connected");

    let mut rx = state.screen_sync_rx.resubscribe();
    while let Ok(screen_sync) = rx.recv().await {
        let bytes = screen_sync.encode_to_vec();
        if ws.send(Message::Binary(bytes)).await.is_err() {
            break;
        }
    }

    info!("Websocket closed");
}

async fn random_color_loop(shared_state: Arc<AppState>, screen_sync_tx: Sender<ScreenSync>) {
    let mut interval = interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;
        {
            let mut fb = shared_state.framebuffer.write().await;
            fb.fill_with_rainbow();
        }

        let fb = shared_state.framebuffer.read().await;

        screen_sync_tx
            .send(fb.deref().into())
            .expect("Failed to send ScreenSync to channel");
    }
}
