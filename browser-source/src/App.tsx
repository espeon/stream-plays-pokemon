import { useEffect, useRef, useState } from "react";
import { useGameStream } from "./useGameStream";
import type { GameState, PartyPokemon } from "./types";
import FrameWorker from "./frame-worker?worker";
import InputRow from "./components/input";

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
  const { state, party, connected, frameCallbackRef } = useGameStream();
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
    <div
      className="relative bg-zinc-950 text-white overflow-hidden select-none font-sans"
      style={{ width: SCREEN_W, height: SCREEN_H }}
    >
      {/* Game viewport — top left */}
      <div
        className="absolute top-4 left-4 rounded-xl bg-black rounded-br-xl overflow-hidden"
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
          <div className="absolute inset-0 flex items-center justify-center bg-black/80">
            <p className="text-white/50 text-sm tracking-widest uppercase">connecting…</p>
          </div>
        )}
        {/* fps — unobtrusive top-left corner of game */}
        {fps !== null && (
          <div className="absolute top-2 left-2 flex flex-col gap-px text-[10px] tabular-nums text-foreground pointer-events-none leading-none">
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
        <div className="flex-1 rounded-xl bg-zinc-900 border border-white/8 flex items-center justify-center">
          <span className="text-white/20 text-xs uppercase tracking-widest">map</span>
        </div>

        {/* Inputs + stats */}
        <div className="flex-1 rounded-xl bg-zinc-900/60 border border-white/8 flex flex-col gap-2 p-4 overflow-hidden">
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
      <div className="flex items-center gap-2 flex-shrink-0">
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
      <div className="flex gap-4 text-[11px] tabular-nums text-white/35 flex-shrink-0">
        <span>{state.total_inputs.toLocaleString()} inputs</span>
        <span>{uptimeParts.join(":")}</span>
      </div>
    </>
  );
}

function statusLabel(status: number): string {
  if (status === 0) return "";
  if ((status & 0x07) > 0) return "SLP";
  if (status & 0x08) return "PSN";
  if (status & 0x10) return "BRN";
  if (status & 0x20) return "FRZ";
  if (status & 0x40) return "PAR";
  if (status & 0x80) return "PSN"; // bad poison
  return "";
}

function PokemonCard({ mon }: { mon: PartyPokemon }) {
  const hpPct = mon.max_hp > 0 ? mon.current_hp / mon.max_hp : 0;
  const fainted = mon.current_hp === 0;
  const hpColor = hpPct > 0.5 ? "bg-emerald-500" : hpPct > 0.2 ? "bg-yellow-400" : "bg-red-500";
  const status = statusLabel(mon.status);

  return (
    <div className={`flex flex-col gap-1.5 rounded-lg bg-zinc-800/70 border border-white/8 p-3 flex-1 min-w-0 ${fainted ? "opacity-40" : ""}`}>
      <div className="flex items-baseline gap-1.5 min-w-0">
        <span className="font-semibold text-sm truncate">{mon.nickname || `#${mon.species}`}</span>
        <span className="text-white/40 text-xs tabular-nums flex-shrink-0">Lv{mon.level}</span>
        {status && (
          <span className="text-[10px] px-1 py-px rounded bg-yellow-800/60 text-yellow-300 flex-shrink-0">{status}</span>
        )}
      </div>
      <div className="flex items-center gap-2">
        <div className="flex-1 h-1.5 rounded-full bg-zinc-700 overflow-hidden">
          <div className={`h-full rounded-full ${hpColor}`} style={{ width: `${hpPct * 100}%` }} />
        </div>
        <span className="text-[10px] tabular-nums text-white/40 flex-shrink-0">
          {mon.current_hp}/{mon.max_hp}
        </span>
      </div>
    </div>
  );
}

function PartyPanel({ party }: { party: PartyPokemon[] }) {
  if (party.length === 0) {
    return (
      <div className="w-full h-full flex items-center justify-center rounded-xl bg-zinc-900/60 border border-white/8">
        <span className="text-white/15 text-xs uppercase tracking-widest">party</span>
      </div>
    );
  }

  return (
    <div className="w-full h-full flex gap-3 items-stretch">
      {party.map((mon, i) => (
        <PokemonCard key={i} mon={mon} />
      ))}
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
