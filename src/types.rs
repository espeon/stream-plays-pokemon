use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Anarchy,
    Democracy,
}

#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    Frame(Vec<u8>),
    Audio(Vec<u8>),
    State(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRecord {
    pub user: String,
    pub input: String,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub mode: Mode,
    pub queue_depth: usize,
    pub recent_inputs: Vec<InputRecord>,
    pub votes: HashMap<String, usize>,
    pub vote_time_remaining_ms: u64,
    pub mode_votes: HashMap<String, usize>,
    pub uptime_seconds: u64,
    pub total_inputs: u64,
    pub emulator_fps: f64,
}
