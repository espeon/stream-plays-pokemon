export type Mode = "anarchy" | "democracy";

export interface InputRecord {
  user: string;
  input: string;
  ts: number;
}

export interface GameState {
  mode: Mode;
  queue_depth: number;
  recent_inputs: InputRecord[];
  votes: Record<string, number>;
  vote_time_remaining_ms: number;
  mode_votes: Record<string, number>;
  uptime_seconds: number;
  total_inputs: number;
  emulator_fps: number;
}

export interface PartyPokemon {
  species: number;
  nickname: string;
  level: number;
  current_hp: number;
  max_hp: number;
  status: number;
  moves: [number, number, number, number];
}
