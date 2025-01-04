use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::{State, WebSocketUpgrade},
    routing::{get, get_service},
    Router,
};
use tokio::net::TcpListener;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};
use tracing::info;

use crate::{
    app_state::AppState,
    http_server::{current_screen::get_current_screen, websocket::handle_websocket},
};

mod current_screen;
mod websocket;

pub async fn run_http_server(
    shared_state: Arc<AppState>,
    listener_address: &str,
) -> anyhow::Result<()> {
    let app = build_router(shared_state);

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .with_context(|| format!("Failed to bind to web listener address {listener_address}"))?;

    info!("Starting HTTP server at http://localhost:3000");
    axum::serve(listener, app)
        .await
        .context("Failed to start server")?;

    Ok(())
}

fn build_router(shared_state: Arc<AppState>) -> Router {
    Router::new()
        .route_service("/", get_service(ServeFile::new("./web/static/index.html")))
        .route(
            "/ws",
            get(
                |ws: WebSocketUpgrade, state: State<Arc<AppState>>| async move {
                    ws.on_upgrade(move |socket| handle_websocket(socket, state))
                },
            ),
        )
        .route("/api/current-screen", get(get_current_screen))
        .nest_service("/static", get_service(ServeDir::new("./web/static")))
        // TODO: Try to restrict
        .layer(CorsLayer::permissive())
        .with_state(shared_state)
}
