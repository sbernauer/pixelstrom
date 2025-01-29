use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::app_state::AppState;

pub async fn get_current_screen_size(state: State<Arc<AppState>>) -> Json<Value> {
    let fb = state.framebuffer.read().await;
    let width = fb.width();
    let height = fb.height();

    Json(json!({ "width": width, "height": height }))
}
