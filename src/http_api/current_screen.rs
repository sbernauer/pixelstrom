use std::{ops::Deref, sync::Arc};

use axum::{body::Bytes, extract::State, http::header, response::IntoResponse};
use prost::Message;

use crate::{app_state::AppState, ScreenSync};

pub async fn get_current_screen(state: State<Arc<AppState>>) -> impl IntoResponse {
    let screen_sync: ScreenSync = state.framebuffer.read().await.deref().into();

    (
        [(header::CONTENT_TYPE, "application/x-protobuf")],
        Bytes::from(screen_sync.encode_to_vec()),
    )
}
