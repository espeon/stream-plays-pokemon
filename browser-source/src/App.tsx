import { useEffect, useRef, useState } from "react";
import { useGameStream } from "./useGameStream";
import type { GameState, PlayerLocation } from "./types";
import FrameWorker from "./frame-worker?worker";
import InputRow from "./components/input";
import { PartyPanel } from "./components/pokemon-card";
import { lookupLocation, MAPSEC, REGION_MAP_TILE_W, REGION_MAP_TILE_H, emeraldMapToObject } from "./emerald-map-data";

const SCALE = 5;
const GBA_W = 240 * SCALE; // 1200
const GBA_H = 160 * SCALE; // 800
const SCREEN_W = 1920;
const SCREEN_H = 1080;
const MARGIN = 16;
const RIGHT_W = SCREEN_W - GBA_W - MARGIN; // 720
const BOTTOM_H = SCREEN_H - GBA_H - MARGIN; // 280

// Module-level singleton: survives Strict Mode's mount→unmount→remount cycle.
let frameWorker: Worker | null = null;

function getOrCreateWorker(canvas: HTMLCanvasElement): Worker {
  if (frameWorker) return frameWorker;
  const worker = new FrameWorker();
  const offscreen = canvas.transferControlToOffscreen();
  worker.postMessage({ type: "init", data: offscreen }, [offscreen]);
  frameWorker = worker;
  return worker;
}

export default function App() {
  const { state, party, location, connected, frameCallbackRef, unmute, audioReady } = useGameStream();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frameTimesRef = useRef<number[]>([]);
  const [fps, setFps] = useState<number | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const worker = getOrCreateWorker(canvas);
    worker.onmessage = (ev: MessageEvent<{ type: string; ts: number }>) => {
      if (ev.data.type !== "frameDone") return;
      const times = frameTimesRef.current;
      times.push(ev.data.ts);
      while (times.length > 60) times.shift();
      if (times.length >= 2) {
        const elapsed = (times[times.length - 1] - times[0]) / 1000;
        setFps(Math.round((times.length - 1) / elapsed));
      }
    };
    frameCallbackRef.current = (jpeg: ArrayBuffer) => {
      worker.postMessage({ type: "frame", data: jpeg }, [jpeg]);
    };
    return () => {
      frameCallbackRef.current = null;
      worker.onmessage = null;
    };
  }, [frameCallbackRef]);

  return (
    <div className="relative flex items-center justify-center h-screen w-screen bg-background">
    <div
      className="relative bg-background text-white overflow-hidden select-none font-sans"
      style={{ width: SCREEN_W, height: SCREEN_H }}
    >
      {/* Game viewport — top left */}
      <div
        className="absolute top-4 left-4 rounded-xl bg-muted rounded-br-xl overflow-hidden"
        style={{ width: GBA_W, height: GBA_H }}
      >
        <canvas
          ref={canvasRef}
          width={240}
          height={160}
          className="w-full h-full"
          style={{ imageRendering: "pixelated" }}
        />
        {!connected && (
          <div className="absolute inset-0 flex items-center justify-center bg-muted/80">
            <p className="text-white/50 text-sm tracking-widest uppercase">connecting…</p>
          </div>
        )}
        {/* fps — unobtrusive top-left corner of game, clickable to unmute */}
        {fps !== null && (
          <div
            className={`absolute top-2 left-2 flex flex-col gap-px text-[10px] tabular-nums leading-none cursor-pointer hover:bg-white/10 rounded px-1 ${audioReady ? 'text-foreground pointer-events-none' : 'text-muted-foreground'}`}
            onClick={audioReady ? undefined : unmute}
          >
            <span>{fps} fps</span>
            {state && <span>{state.emulator_fps.toFixed(1)} srv</span>}
          </div>
        )}
      </div>

      {/* Right panel — map top, inputs bottom */}
      <div
        className="absolute top-0 right-0 flex flex-col gap-4 p-4"
        style={{ width: RIGHT_W, height: SCREEN_H }}
      >
        {/* Map */}
        <div className="flex-1 rounded-xl bg-muted border border-white/8 overflow-hidden">
          <MapPanel location={location} />
        </div>

        {/* Inputs + stats */}
        <div className="flex-1 rounded-xl bg-muted/60 border border-white/8 flex flex-col gap-2 p-4 overflow-hidden">
          <InputsPanel state={state} />
        </div>
      </div>

      {/* Bottom strip — party, full width */}
      <div
        className="absolute bottom-0 left-0 py-4 pl-4"
        style={{ width: GBA_W + MARGIN, height: BOTTOM_H }}
      >
        <PartyPanel party={party} />
      </div>
      </div>
    </div>
  );
}

function InputsPanel({ state }: { state: GameState | null }) {
  if (!state) return null;
  const isAnarchy = state.mode === "anarchy";

  const uptimeParts = [
    Math.floor(state.uptime_seconds / 3600),
    Math.floor((state.uptime_seconds % 3600) / 60),
    state.uptime_seconds % 60,
  ].map((n) => String(n).padStart(2, "0"));

  return (
    <>
      {/* Mode + queue/vote */}
      <div className="flex items-center gap-2 shrink-0">
        <div
          className={`px-2 py-0.5 text-sm rounded font-medium tracking-widest uppercase ${
            isAnarchy ? "bg-emerald-700/70 text-white" : "bg-violet-700/70 text-white"
          }`}
        >
          {state.mode}
        </div>
        {isAnarchy && state.queue_depth > 0 && (
          <span className="text-white/35 tabular-nums text-sm">{state.queue_depth} queued</span>
        )}
        {!isAnarchy && <VoteTally state={state} />}
      </div>

      {/* Recent inputs feed */}
      <div className="flex flex-col gap-0.5 flex-1 overflow-hidden">
        {state.recent_inputs.slice(0, 18).map((r, i) => (
          <InputRow key={i} record={r} index={i} />
        ))}
      </div>

      {/* Stats */}
      <div className="flex gap-4 text-[11px] tabular-nums text-white/35 shrink-0">
        <span>{state.total_inputs.toLocaleString()} inputs</span>
        <span>{uptimeParts.join(":")}</span>
      </div>
    </>
  );
}

function mapsecColor(id: number, active: boolean): string {
  if (active) return "#f97316";
  if (id <= 6)  return "#d4d4aa";  // towns
  if (id <= 15) return "#e8e8c0";  // cities
  if ((id >= 22 && id <= 24) || (id >= 39 && id <= 49)) return "#4a8fb5"; // sea routes
  if (id >= 50 && id <= 54) return "#2d6080"; // underwater
  if (id >= 16 && id <= 49) return "#4a7c50"; // land routes
  return "#6b6b6b"; // dungeons / special
}

function MapPanel({ location }: { location: PlayerLocation | null }) {
  const [imgError, setImgError] = useState(false);

  const entry = location ? lookupLocation(location.map_bank, location.map_num) : null;
  const entryObj = location ? emeraldMapToObject(location.map_bank, location.map_num) : null;

  const activeMapsecId = entry
    ? (Object.entries(MAPSEC).find(([, v]) => v === entry)?.[0] ?? null)
    : null;

  const activeName = entry
    ? entry.name
    : location
    ? `Map ${location.map_bank}:${location.map_num}`
    : "—";

  const mapId = location
    ? ((location.map_bank << 8) | location.map_num).toString(16).padStart(4, "0").toUpperCase()
    : null;

  useEffect(() => {
    setImgError(false);
  }, [mapId]);

  const VW = REGION_MAP_TILE_W;
  const VH = REGION_MAP_TILE_H;

  return (
    <div className="w-full h-full flex flex-col relative">
      <div className="px-4 pt-3 pb-0 shrink-0 z-100 bg-linear-to-b from-muted via-muted/90 to-transparent">
        <div className="text-xl uppercase font-text font-medium tracking-widest text-foreground">{entryObj?.mainArea}</div>
        <div className="text-base uppercase tracking-widest text-muted-foreground">{entryObj?.mapName} {entryObj?.floor ? `(${entryObj.floor < 0 ? `B${entryObj.floor}`:entryObj.floor}F)` : ""}<span className="text-muted">a</span></div>
      </div>

      <div className="flex-1 relative overflow-hidden">
        {/* Current area map */}
        <div className="absolute w-full h-full flex items-center justify-center p-3 ml-18 pr-18">
          {mapId && !imgError ? (
            <img
              src={`/maps/${mapId}.png`}
              onError={() => setImgError(true)}
              style={{ imageRendering: "pixelated", minHeight: "100%", maxWidth: "130%", maxHeight: "100%", objectFit: "contain" }}
              className="rounded-xl"
              alt={activeName}
            />
          ) : (
            <span className="text-white/15 text-xs uppercase tracking-widest">
              {mapId ? "map unavailable" : "—"}
            </span>
          )}
        </div>
        {/* World map overlay — top left */}
        <div className="absolute top-2 left-2 h-32 rounded opacity-80 overflow-hidden border bg-muted/80">
          <svg
            viewBox={`0 0 ${VW} ${VH}`}
            className="w-full h-full"
            preserveAspectRatio="xMidYMid meet"
          >
            <rect x={0} y={0} width={VW} height={VH} fill="#1e3a5f" />
            {Object.entries(MAPSEC).map(([idStr, sec]) => {
              const id = Number(idStr);
              const isActive = idStr === activeMapsecId;
              return (
                <rect
                  key={id}
                  x={sec.tx}
                  y={sec.ty}
                  width={sec.tw}
                  height={sec.th}
                  fill={mapsecColor(id, isActive)}
                  opacity={isActive ? 1 : 0.85}
                />
              );
            })}
            {entry && (
              <rect
                x={entry.tx}
                y={entry.ty}
                width={entry.tw}
                height={entry.th}
                fill="none"
                stroke="white"
                strokeWidth={0.15}
              />
            )}
          </svg>
        </div>
      </div>
    </div>
  );
}

function VoteTally({ state }: { state: GameState }) {
  const sorted = Object.entries(state.votes).sort((a, b) => b[1] - a[1]).slice(0, 5);
  const total = Object.values(state.votes).reduce((s, v) => s + v, 0);
  const secs = Math.ceil(state.vote_time_remaining_ms / 1000);

  return (
    <div className="flex items-center gap-3 text-sm">
      {sorted.map(([btn, count]) => (
        <div key={btn} className="flex flex-col items-center leading-tight">
          <span className="font-semibold text-white uppercase">{btn}</span>
          <span className="text-[10px] text-white/40">
            {total > 0 ? Math.round((count / total) * 100) : 0}%
          </span>
        </div>
      ))}
      <span className="text-white/30 tabular-nums ml-1">{secs}s</span>
    </div>
  );
}
