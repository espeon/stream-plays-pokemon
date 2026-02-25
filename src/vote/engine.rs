use std::collections::VecDeque;

use crate::config::InputConfig;
use crate::input::parser::parse_chat_message;
use crate::input::types::{ChatMessage, GbaButton};
use crate::types::{InputRecord, Mode};

use super::anarchy::AnarchyQueue;

const RECENT_INPUTS_MAX: usize = 20;
const ANARCHY_QUEUE_CAPACITY: usize = 64;

pub struct VoteEngine {
    pub mode: Mode,
    pub total_inputs: u64,
    queue: AnarchyQueue,
    recent_inputs: VecDeque<InputRecord>,
}

impl VoteEngine {
    pub fn new(config: &InputConfig) -> Self {
        let mode = if config.default_mode == "democracy" { Mode::Democracy } else { Mode::Anarchy };
        let start_throttle = config.start_throttle_secs.unwrap_or(5);
        Self {
            mode,
            total_inputs: 0,
            queue: AnarchyQueue::new(config.rate_limit_ms, start_throttle, ANARCHY_QUEUE_CAPACITY),
            recent_inputs: VecDeque::new(),
        }
    }

    pub fn submit(&mut self, msg: ChatMessage) {
        let Some(input) = parse_chat_message(&msg.text) else { return };
        self.queue.submit(&msg, &input);
    }

    /// Called each emulator frame — returns the next button to press, if any.
    pub fn pop_next_input(&mut self) -> Option<(GbaButton, String)> {
        let result = self.queue.pop()?;
        self.total_inputs += 1;
        let record = InputRecord {
            user: result.1.clone(),
            input: result.0.as_str().to_string(),
            ts: chrono::Utc::now().timestamp_millis(),
        };
        self.recent_inputs.push_front(record);
        if self.recent_inputs.len() > RECENT_INPUTS_MAX {
            self.recent_inputs.pop_back();
        }
        Some(result)
    }

    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    pub fn recent_inputs(&self) -> Vec<InputRecord> {
        self.recent_inputs.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InputConfig;

    fn config() -> InputConfig {
        InputConfig {
            default_mode: "anarchy".to_string(),
            democracy_window_secs: 10,
            rate_limit_ms: 0,
            mode_switch_threshold: 0.75,
            mode_switch_cooldown_secs: 300,
            start_throttle_secs: Some(5),
        }
    }

    fn msg(user: &str, text: &str) -> ChatMessage {
        ChatMessage { user: user.to_string(), text: text.to_string(), ts: 0 }
    }

    #[test]
    fn test_valid_input_queued_and_popped() {
        let mut engine = VoteEngine::new(&config());
        engine.submit(msg("alice", "a"));
        let result = engine.pop_next_input();
        assert_eq!(result, Some((GbaButton::A, "alice".to_string())));
    }

    #[test]
    fn test_invalid_input_ignored() {
        let mut engine = VoteEngine::new(&config());
        engine.submit(msg("alice", "notacommand"));
        assert_eq!(engine.pop_next_input(), None);
    }

    #[test]
    fn test_recent_inputs_recorded() {
        let mut engine = VoteEngine::new(&config());
        engine.submit(msg("alice", "a"));
        engine.pop_next_input();
        let recent = engine.recent_inputs();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].user, "alice");
        assert_eq!(recent[0].input, "a");
    }

    #[test]
    fn test_total_inputs_increments() {
        let mut engine = VoteEngine::new(&config());
        engine.submit(msg("alice", "a"));
        engine.submit(msg("bob", "b"));
        engine.pop_next_input();
        engine.pop_next_input();
        assert_eq!(engine.total_inputs, 2);
    }

    #[test]
    fn test_queue_depth_reflects_pending() {
        let mut engine = VoteEngine::new(&config());
        engine.submit(msg("alice", "a"));
        engine.submit(msg("bob", "b"));
        assert_eq!(engine.queue_depth(), 2);
        engine.pop_next_input();
        assert_eq!(engine.queue_depth(), 1);
    }
}
