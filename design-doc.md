# Twitch Plays GBA — Design Document

## Overview

A "Twitch Plays"-style system where Streamplace chat collectively controls a Game Boy Advance emulator playing Pokémon Emerald. The system runs a Rust-native GBA emulator, streams game frames and audio to a React-based OBS browser source, and outputs to Streamplace via WHIP.

### Design Goals

- Zero OBS configuration beyond adding a single browser source
- Low-latency input-to-frame pipeline (chat input → emulator → viewer screen)
- Support for both anarchy and democracy input modes
- Crash recovery and persistent save state management
- Clean separation: Rust handles emulation + game logic, React handles presentation

---

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                     rust backend                        │
│                                                         │
│  streamplace chat ws ──→ input parser ──→ vote engine   │
│                                              │          │
│                                              ▼          │
│                                     GBA emulator        │
│                                    (RustBoyAdvance-NG)  │
│                                       │        │        │
│                                    frames    audio      │
│                                       │        │        │
│                                       ▼        ▼        │
│                                  JPEG encode  PCM       │
│                                       │        │        │
│                              websocket server           │
│                              (binary frames + audio     │
│                               + JSON state)             │
│                                       │                 │
│  save state manager                   │                 │
│  admin interface                      │                 │
└───────────────────────────────────────┼─────────────────┘
                                        │
                              localhost websocket
                                        │
                                        ▼
┌────────────────────────────────────────────────────────┐
│              OBS browser source (react app)              │
│                                                         │
│  websocket client                                       │
│      │         │           │                            │
│      ▼         ▼           ▼                            │
│  <canvas>   WebAudio    overlay UI                      │
│  (game      (game       (votes, inputs,                 │
│   frames)    audio)      mode, pokemon status)          │
│                                                         │
│  ──→ OBS captures entire browser source ──→ WHIP out    │
└────────────────────────────────────────────────────────┘
```

---

## Components

### 1. Rust Backend

The backend is a single Rust binary responsible for emulation, input processing, and frame/audio delivery.

#### GBA Emulator (RustBoyAdvance-NG)

- Runs Pokémon Emerald ROM headless at native 60fps
- Exposes frame callback → raw RGBA pixels (240×160)
- Exposes audio callback → PCM samples (32768 Hz stereo, s16)
- Accepts joypad input programmatically per frame

#### Input Parser

Parses incoming Streamplace chat messages into valid GBA inputs.

**Valid inputs (case-insensitive):**

| Input | GBA Button |
|-------|-----------|
| `a` | A |
| `b` | B |
| `up`, `down`, `left`, `right` | D-pad |
| `start` | Start |
| `select` | Select |
| `l`, `r` | Shoulder buttons |

**Democracy mode extensions:**

| Input | Meaning |
|-------|---------|
| `right3` | Press right 3 times |
| `a2` | Press A twice |
| `wait` | Do nothing this round |

Messages that don't match a valid input are silently ignored (no error response in chat).

#### Vote Engine

Two modes, switchable at runtime:

**Anarchy Mode (default)**
- Every valid input is queued immediately
- Inputs are consumed one per frame (60 inputs/sec max)
- Per-user rate limit: 1 input per 200ms (5 inputs/sec) to prevent single-user spam
- Queue depth cap of ~30 inputs; excess inputs are dropped (oldest first)

**Democracy Mode**
- Voting window: 10 seconds (configurable)
- Each user gets one vote per window
- At window close, the input with the most votes is executed
- Ties broken randomly
- Compound inputs (e.g. `right3`) are executed sequentially across frames
- `wait` is a valid vote — if it wins, nothing happens

**Mode Switching**
- Chat votes `anarchy` or `democracy` — tracked as a rolling tally
- If >75% of recent voters (last 60 seconds) vote for the other mode, it switches
- Cooldown of 5 minutes between switches
- Admin can force-switch via admin interface

#### Frame Pipeline

1. Emulator produces raw RGBA frame (240×160 = 153,600 bytes)
2. Upscale to 720×480 (3x integer scale) via nearest-neighbor
3. JPEG encode at quality 85 (~15-30KB per frame)
4. Send as binary websocket message

At 60fps this is roughly 1-2MB/s over localhost — well within websocket bandwidth.

#### Audio Pipeline

1. Emulator produces PCM samples (s16, stereo, 32768 Hz)
2. Buffer into chunks (~20ms worth = ~1310 samples = ~5240 bytes)
3. Send as binary websocket message with a type prefix byte to distinguish from frames

Alternatively, encode as Opus in Rust before sending to reduce bandwidth, but raw PCM is simpler and the browser source runs on localhost anyway.

**Message framing:**
```
Frame message:  [0x01] [JPEG bytes...]
Audio message:  [0x02] [PCM bytes (s16le, stereo, 32768Hz)...]
State message:  [0x03] [JSON bytes...]
```

#### State Broadcasting

JSON messages sent at a lower rate (~2-4 Hz) containing:

```json
{
  "mode": "anarchy",
  "queue_depth": 12,
  "recent_inputs": [
    { "user": "ash_ketchum", "input": "a", "ts": 1708000000 },
    { "user": "misty_fan", "input": "right", "ts": 1708000001 }
  ],
  "votes": {
    "a": 5,
    "up": 3,
    "b": 1
  },
  "vote_time_remaining_ms": 4200,
  "mode_votes": { "anarchy": 42, "democracy": 18 },
  "uptime_seconds": 86400,
  "total_inputs": 1482933
}
```

#### Save State Manager

- Auto-save every 5 minutes to disk (timestamped: `save_20260224_143000.state`)
- Keep last 48 save states (~4 hours of rolling backups at 5min intervals)
- On startup, detect crash (no clean shutdown marker) and load latest save state
- Admin command to trigger manual save
- Admin command to load a specific save state (with stream pause)

#### Admin Interface

Lightweight HTTP API (or separate websocket channel with auth) for:

- `POST /admin/mode` — force anarchy/democracy
- `POST /admin/save` — trigger save state
- `POST /admin/load` — load specific save state
- `POST /admin/pause` — pause/unpause emulation
- `GET /admin/status` — current state, uptime, stats

Secured with a simple bearer token from env var.

---

### 2. React Browser Source (OBS Overlay)

A React application served as a static page, loaded as an OBS browser source. Handles all rendering — game display, audio playback, and UI overlay.

#### Browser Source Configuration

- URL: `http://localhost:3000` (or wherever the React dev server / static build is served)
- Resolution: 1920×1080 (standard stream resolution)
- OBS browser source captures both video and audio from the page

#### Canvas Renderer

- Receives JPEG frame binary over websocket
- Decodes JPEG → ImageBitmap → draws to `<canvas>`
- Canvas sized to fill the layout (game upscaled with CSS `image-rendering: pixelated`)
- Target: decode + draw in <5ms per frame

```
┌──────────────────────────────────────────┐
│              1920 × 1080                  │
│                                           │
│   ┌─────────────────────┐  ┌──────────┐  │
│   │                     │  │ INPUT    │  │
│   │                     │  │ FEED     │  │
│   │    GAME CANVAS      │  │          │  │
│   │    (centered,       │  │ ash: a   │  │
│   │     integer-scaled) │  │ misty: ↑ │  │
│   │                     │  │ brock: b │  │
│   │                     │  │ ...      │  │
│   └─────────────────────┘  └──────────┘  │
│                                           │
│   ┌──────────────────────────────────┐    │
│   │ MODE: ANARCHY | Queue: 12 | ▓▓░░│    │
│   └──────────────────────────────────┘    │
│                                           │
└──────────────────────────────────────────┘
```

#### Audio Playback

- Receives PCM chunks over websocket
- Feeds into an AudioWorklet or ScriptProcessorNode
- AudioContext sample rate should match emulator output (32768 Hz) or resample
- OBS browser source captures audio output from the page automatically

**Buffering:** maintain a small audio buffer (~100ms) to smooth out delivery jitter. Too much buffer adds latency; too little causes crackle.

#### Overlay UI

React components rendered as DOM elements positioned over/beside the canvas:

- **Input Feed** — scrolling list of recent inputs with usernames, auto-scrolls
- **Mode Indicator** — current mode (ANARCHY / DEMOCRACY) with vote bar showing mode sentiment
- **Vote Display** (democracy mode) — bar chart of current votes with countdown timer
- **Queue Display** (anarchy mode) — current queue depth indicator
- **Stats Bar** — uptime, total inputs processed

Overlay should use a retro/pixel aesthetic consistent with GBA era. Transparent background so OBS composites cleanly.

#### Websocket Client

```typescript
type ServerMessage =
  | { type: 'frame'; data: ArrayBuffer }   // JPEG bytes
  | { type: 'audio'; data: ArrayBuffer }   // PCM bytes
  | { type: 'state'; data: GameState }     // JSON state

// Reconnect logic: exponential backoff, 1s → 2s → 4s → 8s → max 30s
// On disconnect: show "RECONNECTING..." overlay, freeze last frame
```

---

### 3. OBS (Headless)

OBS runs headless with a minimal configuration:

- **Scene: "Game"** — single browser source pointing at the React app
- **Output: WHIP** — configured with Streamplace ingest URL and stream key

OBS handles:
- Compositing (trivial — one source)
- Encoding (h264, whatever profile Streamplace wants)
- WHIP transport to Streamplace

**Headless OBS options:**
- `obs --minimize-to-tray` with pre-configured scene collection
- obs-websocket for programmatic control (start/stop stream)
- Could also use `obs-cmd` CLI tool

The OBS layer is intentionally thin. It's basically a hardware encoder + WHIP client. All the interesting logic is in the Rust backend and React overlay.

---

## Deployment

### Single-Machine Setup

Everything runs on one box:

```
systemd services:
  ├── twitch-plays-gba.service    (rust backend)
  ├── twitch-plays-overlay.service (static file server for react app, e.g. nginx or `serve`)
  └── obs-headless.service         (OBS with WHIP output)
```

**Hardware requirements:**
- CPU: GBA emulation is lightweight (~5% of a modern core). JPEG encoding at 60fps is similarly light.
- GPU: OBS hardware encoding (NVENC/VAAPI) strongly preferred. Software x264 works but adds CPU load.
- RAM: <1GB total for all components
- Network: upload bandwidth for the stream (6-10 Mbps for 1080p)

### Configuration

Environment variables or config file:

```toml
[emulator]
rom_path = "/opt/twitch-plays/emerald.gba"
save_dir = "/opt/twitch-plays/saves/"
target_fps = 60  # output fps (emulator always runs at 60 internally)

[input]
default_mode = "anarchy"
democracy_window_secs = 10
rate_limit_ms = 200
mode_switch_threshold = 0.75
mode_switch_cooldown_secs = 300

[server]
ws_host = "127.0.0.1"
ws_port = 9001
admin_port = 9002
admin_token = "${ADMIN_TOKEN}"

[stream]
jpeg_quality = 85
audio_buffer_ms = 100

[chat]
streamplace_ws_url = "wss://chat.streamplace.example/ws"
streamplace_token = "${STREAMPLACE_TOKEN}"
```

---

## Pokémon Emerald-Specific Considerations

### Input Design

- **Start button throttling:** in anarchy mode, rate-limit Start to 1 press per 5 seconds globally. Menu spam was a defining feature of TPP but can be annoying — make this configurable.
- **Select button:** essentially useless in gen 3. Can be disabled entirely or ignored.
- **L/R:** used for scrolling in menus. Allow but don't prioritize in UI.

### Known Tricky Sections

These parts of Emerald are notoriously difficult for crowd-controlled play:

- **Bike puzzles (Route 119, Trick House):** may need democracy mode enforced
- **Ice puzzles (Sootopolis Gym):** coordinate-based movement, democracy essential
- **Safari Zone:** limited steps, high-stakes navigation
- **Elite Four:** if the run gets here, democracy for item management, anarchy for battles
- **Surf navigation:** easy to get stuck in water routes

Consider an admin "force democracy" command for when the chat is stuck on a section for hours.

### Game-Specific Stats (Stretch Goals)

If you want to get fancy, you can read GBA memory to extract:

- Current party Pokémon (species, level, HP)
- Badge count
- Pokédex count
- Current location
- Play time

RustBoyAdvance-NG should expose memory read access. Known Emerald memory offsets are well-documented by the ROM hacking community. This data could be displayed in the overlay or broadcast in the state JSON.

---

## Implementation Plan

### Phase 1: Core Loop (MVP)
1. Rust binary that runs RustBoyAdvance-NG headless with Emerald ROM
2. Websocket server that sends JPEG frames + PCM audio
3. React app that renders frames to canvas and plays audio
4. Hardcoded test inputs (no chat yet) to verify the pipeline
5. OBS browser source captures the React app, WHIP to Streamplace

**Milestone: game running and visible on Streamplace with no interactivity**

### Phase 2: Chat Integration
1. Connect to Streamplace chat websocket
2. Input parser + anarchy mode vote engine
3. Wire parsed inputs to the emulator
4. React overlay shows input feed

**Milestone: chat can play the game in anarchy mode**

### Phase 3: Democracy + Polish
1. Democracy mode vote engine
2. Mode switching via chat vote
3. Overlay: vote display, mode indicator, stats bar
4. Save state manager (auto-save, crash recovery)
5. Admin HTTP API

**Milestone: full feature parity with classic Twitch Plays Pokémon**

### Phase 4: Stretch Goals
- Memory reading for party/badge/location display
- Input heatmap or analytics dashboard
- Highlight reel: detect "interesting" moments (gym badge earned, blackout, evolution)
- Sound effects for mode switches, milestone events
- Mobile-friendly companion viewer

---

## Open Questions

1. **Streamplace chat protocol** — what does the websocket API look like for reading chat messages? Need: message text, username, timestamp at minimum.
2. **WHIP auth** — how does Streamplace authenticate WHIP ingest? Bearer token? Stream key in URL?
3. **Overlay positioning** — is the React app the only OBS source (full 1920x1080 with game + overlay composed in-browser)? Or is the game one source and overlay another? Single source is simpler.
4. **ROM distribution** — how is the ROM provided? Baked into the deployment, or loaded at runtime via admin API?
5. **Moderation** — any need to moderate chat inputs beyond ignoring non-commands? Ban list for users?
