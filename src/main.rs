use std::{ops::Deref, sync::Arc, time::Duration};

use anyhow::Context;
use ascii_server::AsciiServer;
use prost::bytes::BufMut;
use rand::Rng;
use tokio::{sync::mpsc, time::interval};

use crate::{
    app_state::AppState,
    http_server::run_http_server,
    proto::{web_socket_message::Payload, UserPainting, WebSocketMessage},
};

mod app_state;
mod ascii_server;
mod framebuffer;
mod http_server;
mod statistics;

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
    let max_pixels_per_slot = 10_000;
    let slot_duration = Duration::from_millis(500);

    let app_state = AppState::new(slot_duration, width, height)
        .await
        .context("failed to create app state")?;
    let shared_state = Arc::new(app_state);

    // let shared_state_clone = shared_state.clone();
    // tokio::spawn(async move { rainbow_loop(shared_state_clone).await });

    // let ws_message_tx_clone = shared_state.ws_message_tx.clone();
    // tokio::spawn(
    //     async move { random_client_paints_loop(width, height, ws_message_tx_clone).await },
    // );

    let ascii_server = AsciiServer::new(
        shared_state.clone(),
        ascii_listener_address,
        max_pixels_per_slot,
        slot_duration,
        width,
        height,
    )
    .await
    .context("Failed to start ASCII server")?;
    tokio::spawn(async move { ascii_server.run().await });

    run_http_server(shared_state, http_listener_address).await?;

    Ok(())
}

#[allow(unused)]
async fn rainbow_loop(shared_state: Arc<AppState>) -> anyhow::Result<()> {
    let tx = &shared_state.ws_message_tx;

    let mut interval = interval(Duration::from_millis(20000));
    loop {
        interval.tick().await;

        {
            let mut fb = shared_state.framebuffer.write().await;
            fb.fill_with_rainbow();
        }

        let fb = shared_state.framebuffer.read().await;
        let screen_sync: proto::ScreenSync = fb.deref().into();

        tx.send(WebSocketMessage {
            payload: Some(Payload::ScreenSync(screen_sync)),
        })
        .await
        .context("Failed to send update to websocket message channel")?;
    }
}

const SIZE: u16 = 300;
#[allow(unused)]
async fn random_client_paints_loop(
    width: u16,
    height: u16,
    ws_message_tx: mpsc::Sender<WebSocketMessage>,
) -> anyhow::Result<()> {
    let mut interval = interval(Duration::from_millis(5_000));
    loop {
        interval.tick().await;

        // For some `Send` reasons I'm unable to re-use `rand::thread_rng()`
        let color: u32 = rand::thread_rng().gen();
        let start_x = rand::thread_rng().gen_range(0..width.saturating_sub(SIZE));
        let start_y = rand::thread_rng().gen_range(0..height.saturating_sub(SIZE));

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
            payload: Some(Payload::UserPainting(UserPainting {
                username: "Sebidooo".to_owned(),
                painted,
            })),
        };

        ws_message_tx
            .send(ws_message)
            .await
            .context("Failed to send update to websocket message channel")?;
    }
}
