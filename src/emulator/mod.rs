pub mod audio;
pub mod frame;

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU16, AtomicU32, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use bit::BitIndex;
use parking_lot::Mutex;
use rustboyadvance_ng::keypad::Keys;
use rustboyadvance_ng::prelude::{GameBoyAdvance, GamepakBuilder};
use tokio::sync::broadcast;

use crate::config::EmulatorConfig;
use crate::error::AppError;
use crate::gba_mem::{location::read_location, party::read_party, Gen3Game};
use crate::input::types::GbaButton;
use crate::types::BroadcastMessage;
use crate::vote::engine::VoteEngine;

use audio::{create_audio_pair, drain_chunk, AudioConsumer, SendAudioInterface};
use frame::{encode_jpeg, to_rgb};

const FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 60);
pub const KEYINPUT_ALL_RELEASED: u16 = 0b1111111111;

pub enum EmulatorCommand {
    SaveState,
    LoadState(PathBuf),
    Pause,
    Resume,
    Shutdown,
}

pub struct EmulatorHandle {
    pub cmd_tx: mpsc::SyncSender<EmulatorCommand>,
    /// Current emulator fps * 10 (e.g. 600 = 60.0 fps), updated every second.
    pub fps_x10: Arc<AtomicU32>,
    pub overlay_keys: Arc<AtomicU16>,
}

fn gba_button_to_key(button: GbaButton) -> Keys {
    match button {
        GbaButton::A => Keys::ButtonA,
        GbaButton::B => Keys::ButtonB,
        GbaButton::Up => Keys::Up,
        GbaButton::Down => Keys::Down,
        GbaButton::Left => Keys::Left,
        GbaButton::Right => Keys::Right,
        GbaButton::Start => Keys::Start,
        GbaButton::Select => Keys::Select,
        GbaButton::L => Keys::ButtonL,
        GbaButton::R => Keys::ButtonR,
    }
}

struct LoopArgs {
    bios_path: String,
    rom_path: String,
    save_dir: String,
    target_fps: u32,
    jpeg_quality: u8,
    audio_interface: SendAudioInterface,
    audio_consumer: AudioConsumer,
    vote_engine: Arc<Mutex<VoteEngine>>,
    cmd_rx: mpsc::Receiver<EmulatorCommand>,
    broadcast_tx: broadcast::Sender<BroadcastMessage>,
    fps_x10: Arc<AtomicU32>,
    overlay_keys: Arc<AtomicU16>,
}

pub fn spawn_emulator(
    config: &EmulatorConfig,
    broadcast_tx: broadcast::Sender<BroadcastMessage>,
    jpeg_quality: u8,
    audio_buffer_ms: u64,
    vote_engine: Arc<Mutex<VoteEngine>>,
    overlay_keys: Arc<AtomicU16>,
) -> Result<EmulatorHandle, AppError> {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<EmulatorCommand>(8);
    let (audio_interface, audio_consumer) = create_audio_pair(audio_buffer_ms);
    let fps_x10 = Arc::new(AtomicU32::new(0));

    let args = LoopArgs {
        bios_path: config.bios_path.clone(),
        rom_path: config.rom_path.clone(),
        save_dir: config.save_dir.clone(),
        target_fps: config.target_fps,
        jpeg_quality,
        audio_interface,
        audio_consumer,
        vote_engine,
        cmd_rx,
        broadcast_tx,
        fps_x10: Arc::clone(&fps_x10),
        overlay_keys: Arc::clone(&overlay_keys),
    };

    thread::Builder::new()
        .name("emulator".into())
        .spawn(move || {
            if let Err(e) = run_emulator_loop(args) {
                tracing::error!("emulator thread exited with error: {e}");
            }
        })
        .map_err(AppError::Io)?;

    Ok(EmulatorHandle {
        cmd_tx,
        fps_x10,
        overlay_keys,
    })
}

fn spawn_encode_thread(
    jpeg_quality: u8,
    broadcast_tx: broadcast::Sender<BroadcastMessage>,
) -> mpsc::SyncSender<Vec<u32>> {
    let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u32>>(1);
    thread::Builder::new()
        .name("jpeg-encode".into())
        .spawn(move || loop {
            let raw = match frame_rx.recv() {
                Ok(buf) => buf,
                Err(_) => break,
            };
            let rgb = to_rgb(&raw);
            match encode_jpeg(
                &rgb,
                frame::DISPLAY_WIDTH,
                frame::DISPLAY_HEIGHT,
                jpeg_quality,
            ) {
                Ok(jpeg) => {
                    let _ = broadcast_tx.send(BroadcastMessage::Frame(jpeg));
                }
                Err(e) => tracing::warn!("jpeg encode error: {e}"),
            }
        })
        .expect("failed to spawn jpeg-encode thread");
    frame_tx
}

fn run_emulator_loop(args: LoopArgs) -> Result<(), AppError> {
    let LoopArgs {
        bios_path,
        rom_path,
        save_dir,
        target_fps,
        jpeg_quality,
        audio_interface,
        mut audio_consumer,
        vote_engine,
        cmd_rx,
        broadcast_tx,
        fps_x10,
        overlay_keys,
    } = args;
    let bios = std::fs::read(&bios_path)
        .map_err(AppError::Io)?
        .into_boxed_slice();
    let cartridge = GamepakBuilder::new()
        .file(std::path::Path::new(&rom_path))
        .build()
        .map_err(|e| AppError::Emulator(e.to_string()))?;

    let mut gba = GameBoyAdvance::new(bios, cartridge, audio_interface);
    gba.skip_bios();

    let gen3_game = Gen3Game::detect(&gba.get_game_code());
    if let Some(game) = gen3_game {
        tracing::info!("detected Gen III game: {:?}", game);
    } else {
        tracing::warn!(
            "game code '{}' not recognized as a Gen III game — party data will not be broadcast",
            gba.get_game_code()
        );
    }

    let frame_skip = (60 / target_fps.max(1)).max(1);
    let mut frame_count: u64 = 0;
    let mut paused = false;
    let save_dir = std::path::Path::new(&save_dir);

    let encode_tx = spawn_encode_thread(jpeg_quality, broadcast_tx.clone());

    let mut fps_window_start = Instant::now();
    let mut fps_frame_count = 0u32;

    loop {
        let frame_start = Instant::now();

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                EmulatorCommand::Pause => paused = true,
                EmulatorCommand::Resume => paused = false,
                EmulatorCommand::Shutdown => return Ok(()),
                EmulatorCommand::SaveState => {
                    if let Err(e) = save_state(&gba, save_dir) {
                        tracing::error!("save state failed: {e}");
                    }
                }
                EmulatorCommand::LoadState(path) => match std::fs::read(&path) {
                    Ok(bytes) => {
                        if let Err(e) = gba.restore_state(&bytes) {
                            tracing::error!("restore state failed: {e}");
                        }
                    }
                    Err(e) => tracing::error!("load state read failed: {e}"),
                },
            }
        }

        if paused {
            thread::sleep(Duration::from_millis(16));
            continue;
        }

        let key_state = gba.get_key_state_mut();
        *key_state = overlay_keys.load(Ordering::Relaxed);
        if let Some((button, _user)) = vote_engine.lock().pop_next_input() {
            let key = gba_button_to_key(button);
            key_state.set_bit(key as usize, false); // 0 = pressed
        }

        gba.frame();
        frame_count += 1;
        fps_frame_count += 1;

        let fps_elapsed = fps_window_start.elapsed();
        if fps_elapsed >= Duration::from_secs(1) {
            let fps = fps_frame_count as f64 / fps_elapsed.as_secs_f64();
            fps_x10.store((fps * 10.0).round() as u32, Ordering::Relaxed);
            fps_frame_count = 0;
            fps_window_start = Instant::now();
        }

        if frame_count.is_multiple_of(frame_skip as u64) {
            let raw: Vec<u32> = gba.get_frame_buffer().to_vec();
            let _ = encode_tx.try_send(raw);
        }

        // Broadcast party data at ~1 Hz
        if frame_count.is_multiple_of(60) {
            if let Some(game) = gen3_game {
                let party = read_party(&mut gba, game);
                if let Ok(json) = serde_json::to_vec(&party) {
                    let _ = broadcast_tx.send(BroadcastMessage::Party(json));
                }
            }
        }

        // Broadcast player location at ~6 Hz
        if frame_count.is_multiple_of(10) && gen3_game.is_some() {
            let loc = read_location(&mut gba);
            if let Ok(json) = serde_json::to_vec(&loc) {
                let _ = broadcast_tx.send(BroadcastMessage::Location(json));
            }
        }

        while let Some(chunk) = drain_chunk(&mut audio_consumer) {
            let _ = broadcast_tx.send(BroadcastMessage::Audio(chunk));
        }

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            let remaining = FRAME_DURATION - elapsed;
            if remaining > Duration::from_millis(4) {
                thread::sleep(remaining - Duration::from_millis(4));
            }
            while frame_start.elapsed() < FRAME_DURATION {
                std::hint::spin_loop();
            }
        }
    }
}

fn save_state(gba: &GameBoyAdvance, save_dir: &std::path::Path) -> Result<(), AppError> {
    let bytes = gba
        .save_state()
        .map_err(|e| AppError::SaveState(e.to_string()))?;
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let path = save_dir.join(format!("save_{ts}.state"));
    std::fs::write(&path, &bytes).map_err(AppError::Io)?;
    tracing::info!("saved state to {}", path.display());
    Ok(())
}
