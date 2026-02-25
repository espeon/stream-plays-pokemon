pub mod admin;
pub mod ws_handler;

use axum::{routing::get, Router};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::types::BroadcastMessage;

use admin::AdminState;
use ws_handler::ws_handler;

pub fn build_game_router(broadcast_tx: broadcast::Sender<BroadcastMessage>) -> Router {
    let cors = CorsLayer::new().allow_origin(Any);

    Router::new()
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(broadcast_tx)
}

pub fn build_admin_router(admin_state: AdminState) -> Router {
    use axum::{middleware, routing::post};
    use axum::routing::get;

    Router::new()
        .route("/admin/status", get(admin::get_status))
        .route("/admin/mode", post(admin::post_mode))
        .route("/admin/save", post(admin::post_save))
        .route("/admin/pause", post(admin::post_pause))
        .layer(middleware::from_fn_with_state(
            admin_state.clone(),
            admin::require_bearer_token,
        ))
        .with_state(admin_state)
}
