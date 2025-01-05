use std::{ops::Deref, sync::Arc, time::Duration};

use anyhow::Context;
use ascii_server::AsciiServer;
use prost::bytes::BufMut;
use rand::Rng;
use tokio::{sync::broadcast::Sender, time::interval};

use crate::{
    app_state::AppState,
    http_server::run_http_server,
    proto::{web_socket_message::Payload, ClientPainting, WebSocketMessage},
};

mod app_state;
mod ascii_server;
mod framebuffer;
mod http_server;

mod proto {
    include!(concat!(env!("OUT_DIR"), "/pixelstrom.rs"));
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let width = 1920;
    let height = 1080;
    let ascii_listener_address = "[::]:1234";
    let http_listener_address = "[::]:3000";

    let app_state = AppState::new(width, height);
    let shared_state = Arc::new(app_state);

    let shared_state_clone = shared_state.clone();
    let web_socket_message_tx_clone = shared_state.web_socket_message_tx.clone();
    tokio::spawn(
        async move { rainbow_loop(shared_state_clone, web_socket_message_tx_clone).await },
    );

    let web_socket_message_tx_clone = shared_state.web_socket_message_tx.clone();
    tokio::spawn(async move {
        random_client_paints_loop(width, height, web_socket_message_tx_clone).await
    });

    let ascii_server =
        AsciiServer::new(shared_state.clone(), ascii_listener_address, width, height)
            .await
            .context("Failed to start ASCII server")?;
    tokio::spawn(async move { ascii_server.run().await });

    run_http_server(shared_state, http_listener_address).await?;

    Ok(())
}

async fn rainbow_loop(
    shared_state: Arc<AppState>,
    web_socket_message_tx: Sender<WebSocketMessage>,
) -> anyhow::Result<()> {
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
            .context("Failed to send ScreenSync to channel")?;
    }
}

const SIZE: u16 = 300;
async fn random_client_paints_loop(
    width: u16,
    height: u16,
    web_socket_message_tx: Sender<WebSocketMessage>,
) -> anyhow::Result<()> {
    let mut interval = interval(Duration::from_millis(50));
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
                painted.put_u16(x);
                painted.put_u16(y);
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
            .context("Failed to send ClientPainting to channel")?;
    }
}
