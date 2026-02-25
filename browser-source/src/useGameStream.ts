import { useEffect, useRef, useState } from "react";
import { createAudioWorkletBlobUrl } from "./audio-worklet";
import type { GameState, PartyPokemon, PlayerLocation } from "./types";

// if we're on the Vite dev server, proxy to the backend dev server
const HOST_LOCATION = window.location.host.endsWith(":5173") ? "localhost:9001" : window.location.host; // e.g. "localhost:8080"

const WS_BASE_URL = `ws://${HOST_LOCATION}/ws`;

// Tag bytes matching the Rust BroadcastMessage framing
const TAG_FRAME = 0x01;
const TAG_AUDIO = 0x02;
const TAG_STATE = 0x03;
const TAG_PARTY = 0x04;
const TAG_LOCATION = 0x05;

// button_id matches GBA KEYINPUT bit positions (0=pressed, 1=released)
const KEY_MAP: Record<string, number> = {
  z: 0,          // A
  x: 1,          // B
  Shift: 2,      // Select
  Enter: 3,      // Start
  ArrowRight: 4,
  ArrowLeft: 5,
  ArrowUp: 6,
  ArrowDown: 7,
  s: 8,          // R
  a: 9,          // L
};

export interface GameStream {
  state: GameState | null;
  party: PartyPokemon[];
  location: PlayerLocation | null;
  connected: boolean;
  isOverlay: boolean;
  frameCallbackRef: React.RefObject<((jpeg: ArrayBuffer) => void) | null>;
}

export function useGameStream(): GameStream {
  const [state, setState] = useState<GameState | null>(null);
  const [party, setParty] = useState<PartyPokemon[]>([]);
  const [location, setLocation] = useState<PlayerLocation | null>(null);
  const [connected, setConnected] = useState(false);
  const frameCallbackRef = useRef<((jpeg: ArrayBuffer) => void) | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Audio refs — created once on first user gesture (AudioContext requires it)
  const audioCtxRef = useRef<AudioContext | null>(null);
  const workletNodeRef = useRef<AudioWorkletNode | null>(null);
  const audioReadyRef = useRef(false);

  const overlayToken = new URLSearchParams(window.location.search).get("token");
  const wsUrl = overlayToken
    ? `${WS_BASE_URL}?token=${encodeURIComponent(overlayToken)}`
    : WS_BASE_URL;

  useEffect(() => {
    let ws: WebSocket;
    let dead = false;
    const pressedKeys = new Set<string>();

    async function initAudio() {
      if (audioReadyRef.current) return;
      audioReadyRef.current = true;
      const ctx = new AudioContext();
      audioCtxRef.current = ctx;
      const blobUrl = createAudioWorkletBlobUrl();
      await ctx.audioWorklet.addModule(blobUrl);
      URL.revokeObjectURL(blobUrl);
      const node = new AudioWorkletNode(ctx, "gba-audio-processor");
      node.connect(ctx.destination);
      workletNodeRef.current = node;
    }

    function connect() {
      ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      ws.binaryType = "arraybuffer";

      ws.onopen = () => {
        if (dead) { ws.close(); return; }
        setConnected(true);
        // Init audio on first connection (counts as user gesture in most browsers
        // since the page load triggered by OBS is considered active)
        initAudio().catch(console.error);
      };

      ws.onclose = () => {
        pressedKeys.clear();
        setConnected(false);
        if (!dead) setTimeout(connect, 1500);
      };

      ws.onerror = () => ws.close();

      ws.onmessage = (ev: MessageEvent<ArrayBuffer>) => {
        const buf = ev.data;
        if (buf.byteLength < 1) return;
        const tag = new Uint8Array(buf, 0, 1)[0];
        const payload = buf.slice(1);

        if (tag === TAG_FRAME) {
          frameCallbackRef.current?.(payload);
        } else if (tag === TAG_AUDIO) {
          const node = workletNodeRef.current;
          if (node) {
            // Transfer as Int16Array — zero-copy with transfer
            const i16 = new Int16Array(payload.slice(0));
            node.port.postMessage(i16, [i16.buffer]);
          }
        } else if (tag === TAG_STATE) {
          try {
            const text = new TextDecoder().decode(payload);
            setState(JSON.parse(text) as GameState);
          } catch {
            // malformed state — ignore
          }
        } else if (tag === TAG_PARTY) {
          try {
            const text = new TextDecoder().decode(payload);
            setParty(JSON.parse(text) as PartyPokemon[]);
          } catch {
            // malformed party — ignore
          }
        } else if (tag === TAG_LOCATION) {
          try {
            const text = new TextDecoder().decode(payload);
            setLocation(JSON.parse(text) as PlayerLocation);
          } catch {
            // malformed location — ignore
          }
        }
      };
    }

    function handleKeyDown(e: KeyboardEvent) {
      const buttonId = KEY_MAP[e.key];
      if (buttonId === undefined) return;
      e.preventDefault();
      if (pressedKeys.has(e.key)) return; // skip browser repeat events
      pressedKeys.add(e.key);
      const currentWs = wsRef.current;
      if (currentWs?.readyState === WebSocket.OPEN) {
        currentWs.send(new Uint8Array([0x06, buttonId]));
      }
    }

    function handleKeyUp(e: KeyboardEvent) {
      const buttonId = KEY_MAP[e.key];
      if (buttonId === undefined) return;
      e.preventDefault();
      pressedKeys.delete(e.key);
      const currentWs = wsRef.current;
      if (currentWs?.readyState === WebSocket.OPEN) {
        currentWs.send(new Uint8Array([0x07, buttonId]));
      }
    }

    if (overlayToken) {
      document.addEventListener("keydown", handleKeyDown);
      document.addEventListener("keyup", handleKeyUp);
    }

    connect();

    return () => {
      dead = true;
      ws?.close();
      audioCtxRef.current?.close();
      if (overlayToken) {
        document.removeEventListener("keydown", handleKeyDown);
        document.removeEventListener("keyup", handleKeyUp);
      }
    };
  }, []);

  return { state, party, location, connected, isOverlay: !!overlayToken, frameCallbackRef };
}
