//! Real-time GBA player for local testing.
//!
//! Runs the emulator at 60fps, pipes JPEG frames to ffplay via stdout,
//! and accepts button inputs via TCP on a control port.
//!
//! Usage (two terminals):
//!
//!   Terminal 1 — start the player and pipe to ffplay:
//!     cargo run --bin play --release -- \
//!       --bios /path/to/gba_bios.bin   \
//!       --rom  /path/to/emerald.gba    \
//!       --port 9876                    \
//!     | ffplay -f mjpeg -i pipe:0 -vf scale=720:480 -an
//!
//!   Terminal 2 — send inputs (one per line):
//!     nc localhost 9876
//!     > a
//!     > up
//!     > right3

use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        mpsc::{self, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use bit::BitIndex;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use rustboyadvance_ng::keypad::Keys;
use rustboyadvance_ng::prelude::{GameBoyAdvance, GamepakBuilder};
use stream_plays_emerald::emulator::audio::{create_audio_pair, AudioConsumer, SAMPLE_RATE};
use stream_plays_emerald::emulator::frame::{encode_jpeg, to_rgb, DISPLAY_HEIGHT, DISPLAY_WIDTH};
use stream_plays_emerald::input::{parse_chat_message, types::GbaButton, types::ParsedInput};

const FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 60);
const KEYINPUT_ALL_RELEASED: u16 = 0b1111111111;

struct Args {
    bios: PathBuf,
    rom: PathBuf,
    port: u16,
}

fn parse_args() -> anyhow::Result<Args> {
    use anyhow::Context;
    let mut args = std::env::args().skip(1);
    let mut bios = None;
    let mut rom = None;
    let mut port = 9876u16;

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--bios" => bios = Some(PathBuf::from(args.next().context("--bios needs a value")?)),
            "--rom" => rom = Some(PathBuf::from(args.next().context("--rom needs a value")?)),
            "--port" => port = args.next().context("--port needs a value")?.parse()?,
            other => anyhow::bail!("unknown flag: {other}"),
        }
    }

    Ok(Args {
        bios: bios.context("--bios is required")?,
        rom: rom.context("--rom is required")?,
        port,
    })
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

/// Spawn a TCP listener that accepts connections and parses lines into GbaButton presses.
/// Each connection gets its own thread; inputs are sent to the emulator via the shared sender.
fn spawn_input_server(port: u16, input_tx: Arc<Mutex<SyncSender<GbaButton>>>) {
    thread::Builder::new()
        .name("input-server".into())
        .spawn(move || {
            let listener = TcpListener::bind(("127.0.0.1", port))
                .unwrap_or_else(|e| panic!("failed to bind input port {port}: {e}"));
            eprintln!("[play] input server listening on 127.0.0.1:{port}");
            eprintln!("[play] connect with: nc localhost {port}");
            eprintln!("[play] type button names (a, b, up, down, left, right, start, select, l, r, right3, a2, ...)");

            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let tx = Arc::clone(&input_tx);
                        thread::spawn(move || handle_input_connection(s, tx));
                    }
                    Err(e) => eprintln!("[play] input accept error: {e}"),
                }
            }
        })
        .expect("failed to spawn input server thread");
}

fn handle_input_connection(stream: TcpStream, input_tx: Arc<Mutex<SyncSender<GbaButton>>>) {
    use std::io::{BufRead, BufReader};
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    eprintln!("[play] input client connected: {peer}");

    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        match parse_chat_message(&line) {
            Some(ParsedInput::Button(btn)) => {
                let _ = input_tx.lock().try_send(btn);
            }
            Some(ParsedInput::Compound(btn, count)) => {
                for _ in 0..count {
                    let _ = input_tx.lock().try_send(btn);
                }
            }
            Some(ParsedInput::Wait) => {}
            Some(ParsedInput::VoteAnarchy | ParsedInput::VoleDemocracy) => {}
            None => eprintln!("[play] unknown input: {line:?}"),
        }
    }

    eprintln!("[play] input client disconnected: {peer}");
}

/// Open a cpal output stream that drains the AudioConsumer ring buffer.
/// Returns the stream (must be kept alive — dropping it stops playback).
///
/// The GBA produces audio at 32768 Hz. If the device doesn't support that rate,
/// we use the device's preferred rate and do nearest-neighbor resampling.
fn start_audio_stream(mut consumer: AudioConsumer) -> anyhow::Result<cpal::Stream> {
    use ringbuf::traits::Consumer as _;

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no audio output device found"))?;

    let supported = device.default_output_config()?;
    let device_rate = supported.sample_rate().0;
    let device_channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    let gba_rate = SAMPLE_RATE as u32;
    eprintln!(
        "[play] audio: device rate {device_rate} Hz, {device_channels}ch, format {sample_format:?}, gba rate {gba_rate} Hz"
    );

    let config = cpal::StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let ratio = gba_rate as f64 / device_rate as f64;

    // macOS CoreAudio reports f32 natively; build the appropriate stream type.
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let mut resample_pos: f64 = 0.0;
            let mut last = [0i16; 2];
            device.build_output_stream(
                &config,
                move |out: &mut [f32], _| {
                    for frame in out.chunks_exact_mut(device_channels) {
                        resample_pos += ratio;
                        while resample_pos >= 1.0 {
                            last[0] = consumer.consumer.try_pop().unwrap_or(last[0]);
                            last[1] = consumer.consumer.try_pop().unwrap_or(last[1]);
                            resample_pos -= 1.0;
                        }
                        frame[0] = last[0] as f32 / i16::MAX as f32;
                        if device_channels > 1 {
                            frame[1] = last[1] as f32 / i16::MAX as f32;
                        }
                    }
                },
                |e| eprintln!("[play] audio stream error: {e}"),
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let mut resample_pos: f64 = 0.0;
            let mut last = [0i16; 2];
            device.build_output_stream(
                &config,
                move |out: &mut [i16], _| {
                    for frame in out.chunks_exact_mut(device_channels) {
                        resample_pos += ratio;
                        while resample_pos >= 1.0 {
                            last[0] = consumer.consumer.try_pop().unwrap_or(last[0]);
                            last[1] = consumer.consumer.try_pop().unwrap_or(last[1]);
                            resample_pos -= 1.0;
                        }
                        frame[0] = last[0];
                        if device_channels > 1 {
                            frame[1] = last[1];
                        }
                    }
                },
                |e| eprintln!("[play] audio stream error: {e}"),
                None,
            )?
        }
        fmt => anyhow::bail!("unsupported audio sample format: {fmt:?}"),
    };

    stream.play()?;
    Ok(stream)
}

/// Spawn a thread that receives raw GBA frame buffers (Vec<u32>), encodes them as JPEG,
/// and writes them to stdout. Runs independently of the emulator timing loop.
fn spawn_encode_thread() -> SyncSender<Vec<u32>> {
    // Capacity 1: if encode falls behind, drop rather than accumulate stale frames.
    let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u32>>(1);

    thread::Builder::new()
        .name("jpeg-encode".into())
        .spawn(move || {
            let mut stdout = std::io::stdout().lock();
            loop {
                let raw = match frame_rx.recv() {
                    Ok(buf) => buf,
                    Err(_) => break, // sender dropped — emulator exited
                };
                let rgb = to_rgb(&raw);
                let jpeg = match encode_jpeg(&rgb, DISPLAY_WIDTH, DISPLAY_HEIGHT, 85) {
                    Ok(j) => j,
                    Err(e) => {
                        eprintln!("[play] jpeg encode error: {e}");
                        continue;
                    }
                };
                if let Err(e) = stdout.write_all(&jpeg).and_then(|_| stdout.flush()) {
                    eprintln!("[play] stdout write error: {e}");
                    break;
                }
            }
        })
        .expect("failed to spawn encode thread");

    frame_tx
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;

    let bios = std::fs::read(&args.bios)?.into_boxed_slice();
    let cartridge = GamepakBuilder::new()
        .file(&args.rom)
        .without_backup_to_file()
        .build()
        .map_err(|e| anyhow::anyhow!("loading ROM: {e}"))?;

    let (audio_capture, audio_consumer) = create_audio_pair(200);
    let mut gba = GameBoyAdvance::new(bios, cartridge, audio_capture);
    gba.skip_bios();

    let _audio_stream = start_audio_stream(audio_consumer)?;

    let (input_tx, input_rx) = mpsc::sync_channel::<GbaButton>(64);
    let input_tx = Arc::new(Mutex::new(input_tx));

    spawn_input_server(args.port, input_tx);
    let frame_tx = spawn_encode_thread();

    eprintln!("[play] running — pipe stdout to ffplay");

    let mut pending: Vec<GbaButton> = Vec::new();

    // FPS + frame timing tracking
    let mut fps_window_start = Instant::now();
    let mut fps_frame_count = 0u32;
    let mut frame_us_max = 0u64;
    let mut frame_us_sum = 0u64;

    loop {
        let frame_start = Instant::now();

        while let Ok(btn) = input_rx.try_recv() {
            pending.push(btn);
        }

        let key_state = gba.get_key_state_mut();
        *key_state = KEYINPUT_ALL_RELEASED;
        if !pending.is_empty() {
            let btn = pending.remove(0);
            let key = gba_button_to_key(btn);
            key_state.set_bit(key as usize, false); // 0 = pressed
        }

        let emu_start = Instant::now();
        gba.frame();
        let emu_us = emu_start.elapsed().as_micros() as u64;

        // Clone the frame buffer and hand it to the encode thread — non-blocking.
        // If the encode thread is busy (channel full), the old frame is dropped.
        let raw: Vec<u32> = gba.get_frame_buffer().to_vec();
        let _ = frame_tx.try_send(raw);

        fps_frame_count += 1;
        frame_us_sum += emu_us;
        frame_us_max = frame_us_max.max(emu_us);

        let fps_elapsed = fps_window_start.elapsed();
        if fps_elapsed >= Duration::from_secs(2) {
            let fps = fps_frame_count as f64 / fps_elapsed.as_secs_f64();
            let avg_us = frame_us_sum / fps_frame_count as u64;
            eprintln!("[play] {fps:.1} fps  |  gba.frame(): avg {avg_us}µs  max {frame_us_max}µs");
            fps_frame_count = 0;
            frame_us_sum = 0;
            frame_us_max = 0;
            fps_window_start = Instant::now();
        }

        // Sleep most of the remaining budget, then spin the last 2ms to avoid
        // OS timer granularity (~10ms on macOS) causing frame rate to slip to 50fps.
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
