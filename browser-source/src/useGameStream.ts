import { useEffect, useRef, useState } from "react";
import { createAudioWorkletBlobUrl } from "./audio-worklet";
import type { GameState, PartyPokemon, PlayerLocation } from "./types";

// if we're on the Vite dev server, proxy to the backend dev server
const HOST_LOCATION = window.location.host.endsWith(":5173") ? "localhost:9001" : window.location.host; // e.g. "localhost:8080"

const WS_URL = `ws://${HOST_LOCATION}/ws`;

// Tag bytes matching the Rust BroadcastMessage framing
const TAG_FRAME = 0x01;
const TAG_AUDIO = 0x02;
const TAG_STATE = 0x03;
const TAG_PARTY = 0x04;
const TAG_LOCATION = 0x05;

export interface GameStream {
  state: GameState | null;
  party: PartyPokemon[];
  location: PlayerLocation | null;
  connected: boolean;
  frameCallbackRef: React.RefObject<((jpeg: ArrayBuffer) => void) | null>;
}

export function useGameStream(): GameStream {
  const [state, setState] = useState<GameState | null>(null);
  const [party, setParty] = useState<PartyPokemon[]>([]);
  const [location, setLocation] = useState<PlayerLocation | null>(null);
  const [connected, setConnected] = useState(false);
  const frameCallbackRef = useRef<((jpeg: ArrayBuffer) => void) | null>(null);

  // Audio refs — created once on first user gesture (AudioContext requires it)
  const audioCtxRef = useRef<AudioContext | null>(null);
  const workletNodeRef = useRef<AudioWorkletNode | null>(null);
  const audioReadyRef = useRef(false);

  useEffect(() => {
    let ws: WebSocket;
    let dead = false;

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
      ws = new WebSocket(WS_URL);
      ws.binaryType = "arraybuffer";

      ws.onopen = () => {
        if (dead) { ws.close(); return; }
        setConnected(true);
        // Init audio on first connection (counts as user gesture in most browsers
        // since the page load triggered by OBS is considered active)
        initAudio().catch(console.error);
      };

      ws.onclose = () => {
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

    connect();

    return () => {
      dead = true;
      ws?.close();
      audioCtxRef.current?.close();
    };
  }, []);

  return { state, party, location, connected, frameCallbackRef };
}
