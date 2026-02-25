use std::sync::{atomic::Ordering, Arc};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::emulator::EmulatorCommand;
use crate::types::{GameState, Mode};

#[derive(Clone)]
pub struct AdminState {
    pub token: String,
    pub game_state: Arc<parking_lot::RwLock<GameState>>,
    pub emulator_fps_x10: Arc<std::sync::atomic::AtomicU32>,
    pub cmd_tx: std::sync::mpsc::SyncSender<EmulatorCommand>,
}

/// Axum middleware: require `Authorization: Bearer <token>` header.
pub async fn require_bearer_token(
    State(admin): State<AdminState>,
    req: Request,
    next: Next,
) -> Response {
    let auth = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    match auth {
        Some(value) if value == format!("Bearer {}", admin.token) => next.run(req).await,
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetModeRequest {
    pub mode: Mode,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub state: GameState,
}

pub async fn get_status(State(admin): State<AdminState>) -> Json<StatusResponse> {
    let mut state = admin.game_state.read().clone();
    state.emulator_fps = admin.emulator_fps_x10.load(Ordering::Relaxed) as f64 / 10.0;
    Json(StatusResponse { state })
}

pub async fn post_mode(
    State(admin): State<AdminState>,
    Json(req): Json<SetModeRequest>,
) -> StatusCode {
    admin.game_state.write().mode = req.mode;
    StatusCode::OK
}

pub async fn post_save(State(admin): State<AdminState>) -> StatusCode {
    match admin.cmd_tx.try_send(EmulatorCommand::SaveState) {
        Ok(()) => StatusCode::ACCEPTED,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

pub async fn post_pause(State(admin): State<AdminState>) -> StatusCode {
    match admin.cmd_tx.try_send(EmulatorCommand::Pause) {
        Ok(()) => StatusCode::ACCEPTED,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        middleware,
        routing::{get, post},
        Router,
    };
    use axum_test::TestServer;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_state(token: &str) -> AdminState {
        let game_state = GameState {
            mode: Mode::Anarchy,
            queue_depth: 0,
            recent_inputs: vec![],
            votes: HashMap::new(),
            vote_time_remaining_ms: 0,
            mode_votes: HashMap::new(),
            uptime_seconds: 0,
            total_inputs: 0,
            emulator_fps: 0.0,
        };
        let (cmd_tx, _cmd_rx) = std::sync::mpsc::sync_channel(8);
        AdminState {
            token: token.into(),
            game_state: Arc::new(parking_lot::RwLock::new(game_state)),
            emulator_fps_x10: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            cmd_tx,
        }
    }

    fn build_app(state: AdminState) -> Router {
        let protected = Router::new()
            .route("/admin/status", get(get_status))
            .route("/admin/mode", post(post_mode))
            .route("/admin/save", post(post_save))
            .route("/admin/pause", post(post_pause))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                require_bearer_token,
            ))
            .with_state(state);
        protected
    }

    #[tokio::test]
    async fn test_status_requires_auth() {
        let server = TestServer::new(build_app(make_state("secret"))).unwrap();
        let res = server.get("/admin/status").await;
        assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_status_with_valid_token() {
        let server = TestServer::new(build_app(make_state("secret"))).unwrap();
        let res = server
            .get("/admin/status")
            .add_header(
                axum::http::header::AUTHORIZATION,
                axum::http::HeaderValue::from_static("Bearer secret"),
            )
            .await;
        assert_eq!(res.status_code(), StatusCode::OK);
        let body: serde_json::Value = res.json();
        assert_eq!(body["state"]["mode"], "anarchy");
    }

    #[tokio::test]
    async fn test_status_with_wrong_token() {
        let server = TestServer::new(build_app(make_state("secret"))).unwrap();
        let res = server
            .get("/admin/status")
            .add_header(
                axum::http::header::AUTHORIZATION,
                axum::http::HeaderValue::from_static("Bearer wrongtoken"),
            )
            .await;
        assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_post_mode_changes_state() {
        let state = make_state("tok");
        let server = TestServer::new(build_app(state.clone())).unwrap();
        let res = server
            .post("/admin/mode")
            .add_header(
                axum::http::header::AUTHORIZATION,
                axum::http::HeaderValue::from_static("Bearer tok"),
            )
            .json(&serde_json::json!({"mode": "democracy"}))
            .await;
        assert_eq!(res.status_code(), StatusCode::OK);
        assert_eq!(state.game_state.read().mode, Mode::Democracy);
    }
}
