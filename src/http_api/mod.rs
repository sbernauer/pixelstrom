use std::sync::Arc;

use axum::{
    extract::{State, WebSocketUpgrade},
    routing::{get, get_service},
    Router,
};
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    app_state::AppState,
    http_api::{current_screen::get_current_screen, websocket::handle_websocket},
};

mod current_screen;
mod websocket;

pub fn build_router(shared_state: Arc<AppState>) -> Router {
    Router::new()
        .route_service("/", get_service(ServeFile::new("static/index.html")))
        .route(
            "/ws",
            get(
                |ws: WebSocketUpgrade, state: State<Arc<AppState>>| async move {
                    ws.on_upgrade(move |socket| handle_websocket(socket, state))
                },
            ),
        )
        .route("/api/current-screen", get(get_current_screen))
        .nest_service("/static", get_service(ServeDir::new("./static")))
        .with_state(shared_state)
}
