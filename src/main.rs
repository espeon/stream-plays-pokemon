#![allow(dead_code, unused_imports)]

use std::{collections::HashMap, net::SocketAddr, sync::Arc, sync::atomic::{AtomicU16, Ordering}, time::Instant};

use anyhow::Context;
use parking_lot::{Mutex, RwLock};
use stream_plays_emerald::{
    chat::client::run_chat_client,
    config::Config,
    emulator,
    save::manager::{
        clean_shutdown_marker_exists, find_latest_save, remove_clean_shutdown_marker,
        spawn_auto_save_task, write_clean_shutdown_marker,
    },
    server,
    types::{BroadcastMessage, GameState, Mode},
    vote::engine::VoteEngine,
};
use stream_plays_emerald::server::admin::AdminState;
use stream_plays_emerald::server::ws_handler::WsState;
use tokio::{net::TcpListener, signal, time};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".into());
    let config = Config::from_file(&config_path)
        .with_context(|| format!("failed to load config from {config_path}"))?;

    let (broadcast_tx, _) = tokio::sync::broadcast::channel(2);

    let vote_engine = Arc::new(Mutex::new(VoteEngine::new(&config.input)));

    let game_state = Arc::new(RwLock::new(GameState {
        mode: Mode::Anarchy,
        queue_depth: 0,
        recent_inputs: vec![],
        votes: HashMap::new(),
        vote_time_remaining_ms: 0,
        mode_votes: HashMap::new(),
        uptime_seconds: 0,
        total_inputs: 0,
        emulator_fps: 0.0,
    }));

    let save_dir = std::path::Path::new(&config.emulator.save_dir);
    std::fs::create_dir_all(save_dir).context("failed to create save_dir")?;

    let was_clean_shutdown = clean_shutdown_marker_exists(save_dir);
    if !was_clean_shutdown {
        if let Some(latest) = find_latest_save(save_dir) {
            tracing::warn!(
                "no clean shutdown marker found — possible crash. latest save: {}",
                latest.display()
            );
        }
    }
    remove_clean_shutdown_marker(save_dir).ok();

    let overlay_keys = Arc::new(AtomicU16::new(emulator::KEYINPUT_ALL_RELEASED));

    let emulator_handle = emulator::spawn_emulator(
        &config.emulator,
        broadcast_tx.clone(),
        config.stream.jpeg_quality,
        config.stream.audio_buffer_ms,
        Arc::clone(&vote_engine),
        Arc::clone(&overlay_keys),
    )?;

    if config.emulator.auto_restore {
        if let Some(latest) = find_latest_save(save_dir) {
            tracing::info!("auto-restoring save state: {}", latest.display());
            let _ = emulator_handle.cmd_tx.try_send(emulator::EmulatorCommand::LoadState(latest));
        }
    }

    let start_time = Instant::now();

    let admin_state = AdminState {
        token: config.server.admin_token.clone(),
        game_state,
        emulator_fps_x10: emulator_handle.fps_x10,
        cmd_tx: emulator_handle.cmd_tx.clone(),
    };

    // Broadcast GameState at ~4 Hz with live queue depth, recent inputs, fps, and uptime.
    {
        let game_state = Arc::clone(&admin_state.game_state);
        let fps_x10 = Arc::clone(&admin_state.emulator_fps_x10);
        let vote_engine = Arc::clone(&vote_engine);
        let tx = broadcast_tx.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(time::Duration::from_millis(250));
            loop {
                interval.tick().await;
                let mut state = game_state.read().clone();
                let engine = vote_engine.lock();
                state.emulator_fps = fps_x10.load(Ordering::Relaxed) as f64 / 10.0;
                state.queue_depth = engine.queue_depth();
                state.recent_inputs = engine.recent_inputs();
                state.total_inputs = engine.total_inputs;
                state.uptime_seconds = start_time.elapsed().as_secs();
                drop(engine);
                if let Ok(json) = serde_json::to_vec(&state) {
                    let _ = tx.send(BroadcastMessage::State(json));
                }
            }
        });
    }

    // Auto-save every 5 minutes
    spawn_auto_save_task(
        emulator_handle.cmd_tx.clone(),
        std::time::Duration::from_secs(300),
    );

    // Spawn chat client
    {
        let ws_url = config.chat.streamplace_ws_url.clone();
        let engine = Arc::clone(&vote_engine);
        tokio::spawn(async move {
            run_chat_client(ws_url, engine).await;
        });
    }

    let ws_state = WsState {
        broadcast_tx,
        overlay_keys,
        admin_token: config.server.admin_token.clone(),
        allow_anonymous_keyboard: config.server.allow_anonymous_keyboard,
    };
    let game_router = server::build_game_router(ws_state);
    let admin_router = server::build_admin_router(admin_state);

    let ws_addr: SocketAddr = format!("{}:{}", config.server.ws_host, config.server.ws_port)
        .parse()
        .context("invalid ws_host/ws_port")?;
    let admin_addr: SocketAddr = format!("{}:{}", config.server.ws_host, config.server.admin_port)
        .parse()
        .context("invalid admin_port")?;

    let ws_listener = TcpListener::bind(ws_addr).await?;
    let admin_listener = TcpListener::bind(admin_addr).await?;

    tracing::info!("game ws listening on {ws_addr}");
    tracing::info!("admin http listening on {admin_addr}");

    tokio::select! {
        res = axum::serve(ws_listener, game_router) => {
            res.context("game ws server error")?;
        }
        res = axum::serve(admin_listener, admin_router) => {
            res.context("admin server error")?;
        }
        _ = signal::ctrl_c() => {
            tracing::info!("shutting down");
        }
    }

    // Write clean shutdown marker so next startup knows we exited cleanly.
    if let Err(e) = write_clean_shutdown_marker(save_dir) {
        tracing::warn!("failed to write clean shutdown marker: {e}");
    }

    Ok(())
}
