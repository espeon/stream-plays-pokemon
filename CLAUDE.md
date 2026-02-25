# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Check, test, lint (run before committing)
just precommit

# Run a single test
cargo test <test_name>

# Run all lib tests (no ROM required)
cargo test --lib

# Build browser source
just build-ui

# E2E tests (requires ROM + BIOS files)
just e2e
```

## Architecture

**stream-plays-emerald** is a "Twitch Plays"-style system: Streamplace chat collectively controls a GBA emulator (Pokémon Emerald) via a vote engine. The Rust backend streams JPEG frames + PCM audio + JSON state over a WebSocket to a React OBS browser source, which OBS then pushes to Streamplace via WHIP.

### Thread model

The emulator runs in a dedicated `std::thread` (not tokio — needs tight 60fps timing loop). Everything else is tokio async tasks. Communication uses:

- `tokio::sync::broadcast::Sender<BroadcastMessage>` (capacity=2) — spine for frame/audio/state/party to WS clients. Capacity-2 intentionally drops lagging clients.
- `std::sync::mpsc::SyncSender<EmulatorCommand>` — admin commands (save, pause, resume, shutdown) sent to the emulator thread
- `Arc<Mutex<VoteEngine>>` (`parking_lot`) — shared between emulator thread (pops inputs each frame) and tokio tasks (chat client submits)
- `Arc<AtomicU32>` — fps×10 reported from emulator thread to state broadcaster

### WebSocket message framing

All messages are binary with a 1-byte prefix:
- `0x01` — JPEG frame (240×160, quality configurable)
- `0x02` — PCM audio (s16le stereo 32768Hz)
- `0x03` — JSON `GameState`
- `0x04` — JSON `Vec<PartyPokemon>` (Gen III party data, broadcast ~1 Hz)

### GBA memory reading (`src/gba_mem/`)

Runtime game detection via `gba.get_game_code()` (e.g. `"BPEE"` → Emerald). `Gen3Game::detect()` maps game codes to per-game party EWRAM addresses. Gen III pokemon data is XOR-encrypted in RAM: key = `pid ^ ot_id`, applied to 4-byte words over the 48-byte substructure block. Substructure slot order is determined by `pid % 24` (24-entry table in `decrypt.rs`).

### rustboyadvance-ng fork

The upstream crate doesn't expose memory reads publicly (`sysbus` is `pub(crate)`). We maintain a local fork at `../rustboyadvance-ng/` (sibling directory) with one added method: `GameBoyAdvance::debug_read_8(&mut self, addr: u32) -> u8`. `Cargo.toml` uses `path = "../rustboyadvance-ng/core"`.

### Key modules

| Module | Purpose |
|--------|---------|
| `emulator/` | `spawn_emulator()` → emulator thread + JPEG encode thread + `EmulatorHandle` |
| `vote/engine.rs` | `VoteEngine` wraps `AnarchyQueue`, parses chat, tracks recent inputs |
| `vote/anarchy.rs` | Per-user rate limiting, Start button throttle, capacity-bound queue |
| `gba_mem/` | Gen III party reading: game detection, XOR decrypt, charmap, struct parsing |
| `save/manager.rs` | Auto-save rotation (48 files), `.clean_shutdown` crash detection marker |
| `server/admin.rs` | Bearer-token-protected HTTP admin API; `AdminState` holds `cmd_tx` |
| `chat/client.rs` | Streamplace WS client with exponential backoff; discards 1s of backfill on connect |

### Browser source (`browser-source/`)

React app served as OBS browser source at 1920×1080. Uses:
- Module-level `Worker` singleton (`frame-worker.ts`) + `OffscreenCanvas` for JPEG decode off main thread — must be a singleton to survive React StrictMode double-invoke
- `AudioWorkletNode` for low-latency PCM playback
- `useGameStream.ts` exposes `frameCallbackRef` (raw ArrayBuffer → worker) + parsed `GameState`

### Config (`config.toml`)

All fields required except `input.start_throttle_secs` (optional). `admin_token` is a plaintext bearer token. The Streamplace chat WS URL format is `wss://stream.place/api/websocket/<handle>`.
