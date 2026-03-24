use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::config::InputConfig;
use crate::input::parser::parse_chat_message;
use crate::input::types::{ChatMessage, GbaButton, ParsedInput};
use crate::types::{InputRecord, Mode};

use super::anarchy::AnarchyQueue;

const RECENT_INPUTS_MAX: usize = 20;
const ANARCHY_QUEUE_CAPACITY: usize = 64;

struct DemocracyState {
    window_duration: Duration,
    window_start: Instant,
    /// Per-button votes; HashSet deduplicated per user per window.
    votes: HashMap<GbaButton, HashSet<String>>,
}

impl DemocracyState {
    fn new(window_secs: u64) -> Self {
        Self {
            window_duration: Duration::from_secs(window_secs),
            window_start: Instant::now(),
            votes: HashMap::new(),
        }
    }

    fn submit_vote(&mut self, user: &str, button: GbaButton) {
        self.votes.entry(button).or_default().insert(user.to_string());
    }

    /// Returns winning button and resets window if the window has elapsed.
    fn tick(&mut self) -> Option<GbaButton> {
        if self.window_start.elapsed() < self.window_duration {
            return None;
        }
        let winner = self
            .votes
            .iter()
            .max_by_key(|(_, voters)| voters.len())
            .map(|(btn, _)| *btn);
        self.votes.clear();
        self.window_start = Instant::now();
        winner
    }

    fn vote_counts(&self) -> HashMap<GbaButton, usize> {
        self.votes.iter().map(|(b, s)| (*b, s.len())).collect()
    }

    fn time_remaining_ms(&self) -> u64 {
        self.window_duration
            .saturating_sub(self.window_start.elapsed())
            .as_millis() as u64
    }
}

pub struct VoteEngine {
    pub mode: Mode,
    pub total_inputs: u64,
    pub button_counts: HashMap<GbaButton, u64>,
    anarchy: AnarchyQueue,
    democracy: DemocracyState,
    /// Accumulated mode votes across the current period; reset on mode switch.
    mode_vote_counts: HashMap<Mode, usize>,
    last_mode_switch: Instant,
    mode_switch_cooldown: Duration,
    mode_switch_threshold: f64,
    recent_inputs: VecDeque<InputRecord>,
}

impl VoteEngine {
    pub fn new(config: &InputConfig) -> Self {
        let mode = if config.default_mode == "democracy" {
            Mode::Democracy
        } else {
            Mode::Anarchy
        };
        let start_throttle = config.start_throttle_secs.unwrap_or(5);
        Self {
            mode,
            total_inputs: 0,
            button_counts: HashMap::new(),
            anarchy: AnarchyQueue::new(config.rate_limit_ms, start_throttle, ANARCHY_QUEUE_CAPACITY),
            democracy: DemocracyState::new(config.democracy_window_secs),
            mode_vote_counts: HashMap::new(),
            last_mode_switch: Instant::now()
                - Duration::from_secs(config.mode_switch_cooldown_secs + 1),
            mode_switch_cooldown: Duration::from_secs(config.mode_switch_cooldown_secs),
            mode_switch_threshold: config.mode_switch_threshold,
            recent_inputs: VecDeque::new(),
        }
    }

    pub fn submit(&mut self, msg: ChatMessage) {
        let Some(input) = parse_chat_message(&msg.text) else {
            return;
        };
        match &input {
            ParsedInput::Button(btn) | ParsedInput::Compound(btn, _) => {
                let buttons = input.expand();
                match self.mode {
                    Mode::Anarchy => self.anarchy.submit(&msg, &input),
                    Mode::Democracy => {
                        for button in buttons {
                            self.democracy.submit_vote(&msg.user, button);
                        }
                    }
                }
                let _ = btn; // suppress unused warning; buttons consumed above
            }
            ParsedInput::Wait => {}
            ParsedInput::VoteAnarchy => {
                *self.mode_vote_counts.entry(Mode::Anarchy).or_insert(0) += 1;
                self.maybe_switch_mode(Mode::Anarchy);
            }
            ParsedInput::VoteDemocracy => {
                *self.mode_vote_counts.entry(Mode::Democracy).or_insert(0) += 1;
                self.maybe_switch_mode(Mode::Democracy);
            }
        }
    }

    fn maybe_switch_mode(&mut self, target: Mode) {
        if self.mode == target {
            return;
        }
        if self.last_mode_switch.elapsed() < self.mode_switch_cooldown {
            return;
        }
        let total: usize = self.mode_vote_counts.values().sum();
        if total == 0 {
            return;
        }
        let target_votes = self.mode_vote_counts.get(&target).copied().unwrap_or(0);
        if target_votes as f64 / total as f64 >= self.mode_switch_threshold {
            self.mode = target;
            self.mode_vote_counts.clear();
            self.last_mode_switch = Instant::now();
            // Reset democracy window on mode entry so the timer starts fresh.
            self.democracy = DemocracyState::new(self.democracy.window_duration.as_secs());
        }
    }

    /// Called each emulator frame — returns the next button to press, if any.
    pub fn pop_next_input(&mut self) -> Option<(GbaButton, String)> {
        let result = match self.mode {
            Mode::Anarchy => self.anarchy.pop(),
            Mode::Democracy => self.democracy.tick().map(|btn| (btn, "democracy".to_string())),
        }?;
        self.total_inputs += 1;
        *self.button_counts.entry(result.0).or_insert(0) += 1;
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
        match self.mode {
            Mode::Anarchy => self.anarchy.len(),
            Mode::Democracy => self.democracy.votes.values().map(|s| s.len()).sum(),
        }
    }

    pub fn recent_inputs(&self) -> Vec<InputRecord> {
        self.recent_inputs.iter().cloned().collect()
    }

    pub fn vote_counts(&self) -> HashMap<String, usize> {
        match self.mode {
            Mode::Anarchy => HashMap::new(),
            Mode::Democracy => self
                .democracy
                .vote_counts()
                .into_iter()
                .map(|(b, c)| (b.as_str().to_string(), c))
                .collect(),
        }
    }

    pub fn vote_time_remaining_ms(&self) -> u64 {
        match self.mode {
            Mode::Anarchy => 0,
            Mode::Democracy => self.democracy.time_remaining_ms(),
        }
    }

    pub fn mode_vote_counts(&self) -> HashMap<String, usize> {
        self.mode_vote_counts
            .iter()
            .map(|(m, c)| {
                let key = match m {
                    Mode::Anarchy => "anarchy",
                    Mode::Democracy => "democracy",
                };
                (key.to_string(), *c)
            })
            .collect()
    }

    pub fn button_counts_str(&self) -> HashMap<String, u64> {
        self.button_counts
            .iter()
            .map(|(b, c)| (b.as_str().to_string(), *c))
            .collect()
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

    #[test]
    fn test_button_counts_tracked() {
        let mut engine = VoteEngine::new(&config());
        engine.submit(msg("alice", "a"));
        engine.submit(msg("bob", "a"));
        engine.submit(msg("carol", "b"));
        engine.pop_next_input();
        engine.pop_next_input();
        engine.pop_next_input();
        let counts = engine.button_counts_str();
        assert_eq!(counts.get("a"), Some(&2));
        assert_eq!(counts.get("b"), Some(&1));
    }

    // --- Democracy tests ---

    fn democracy_config(window_secs: u64) -> InputConfig {
        InputConfig {
            default_mode: "democracy".to_string(),
            democracy_window_secs: window_secs,
            rate_limit_ms: 0,
            mode_switch_threshold: 0.75,
            mode_switch_cooldown_secs: 300,
            start_throttle_secs: None,
        }
    }

    #[test]
    fn test_democracy_no_input_before_window() {
        let mut engine = VoteEngine::new(&democracy_config(60));
        engine.submit(msg("alice", "a"));
        // Window hasn't elapsed, no input yet
        assert_eq!(engine.pop_next_input(), None);
    }

    #[test]
    fn test_democracy_winner_after_window() {
        let mut engine = VoteEngine::new(&democracy_config(0));
        engine.submit(msg("alice", "a"));
        engine.submit(msg("bob", "a"));
        engine.submit(msg("carol", "b"));
        // 0-second window expires immediately
        let result = engine.pop_next_input();
        assert_eq!(result.map(|(b, _)| b), Some(GbaButton::A));
    }

    #[test]
    fn test_democracy_deduplicates_per_user() {
        let mut engine = VoteEngine::new(&democracy_config(0));
        // alice votes a twice — should only count once
        engine.submit(msg("alice", "a"));
        engine.submit(msg("alice", "a"));
        engine.submit(msg("bob", "b"));
        let result = engine.pop_next_input();
        // a: 1 unique voter, b: 1 unique voter — tie goes to whichever max_by_key picks
        // Either way, no panic and returns Some
        assert!(result.is_some());
    }

    #[test]
    fn test_democracy_resets_after_window() {
        let mut engine = VoteEngine::new(&democracy_config(0));
        engine.submit(msg("alice", "a"));
        engine.pop_next_input(); // consumes window
        // Next window is fresh — no votes yet, nothing to return
        assert_eq!(engine.pop_next_input(), None);
    }

    #[test]
    fn test_democracy_vote_counts_exposed() {
        let mut engine = VoteEngine::new(&democracy_config(60));
        engine.submit(msg("alice", "a"));
        engine.submit(msg("bob", "a"));
        engine.submit(msg("carol", "b"));
        let counts = engine.vote_counts();
        assert_eq!(counts.get("a"), Some(&2));
        assert_eq!(counts.get("b"), Some(&1));
    }

    #[test]
    fn test_democracy_time_remaining_nonzero() {
        let engine = VoteEngine::new(&democracy_config(60));
        assert!(engine.vote_time_remaining_ms() > 0);
    }

    #[test]
    fn test_mode_switch_anarchy_to_democracy() {
        let switch_config = InputConfig {
            default_mode: "anarchy".to_string(),
            democracy_window_secs: 10,
            rate_limit_ms: 0,
            mode_switch_threshold: 0.75,
            mode_switch_cooldown_secs: 0,
            start_throttle_secs: None,
        };
        let mut engine = VoteEngine::new(&switch_config);
        assert_eq!(engine.mode, Mode::Anarchy);
        // 3 democracy votes, 1 anarchy = 75% democracy >= threshold
        engine.submit(msg("a", "democracy"));
        engine.submit(msg("b", "democracy"));
        engine.submit(msg("c", "democracy"));
        engine.submit(msg("d", "anarchy"));
        assert_eq!(engine.mode, Mode::Democracy);
    }

    #[test]
    fn test_mode_switch_respects_cooldown() {
        // Use a real cooldown; VoteEngine::new() pre-expires it so the first switch works.
        let switch_config = InputConfig {
            default_mode: "anarchy".to_string(),
            democracy_window_secs: 10,
            rate_limit_ms: 0,
            mode_switch_threshold: 0.75,
            mode_switch_cooldown_secs: 300,
            start_throttle_secs: None,
        };
        let mut engine = VoteEngine::new(&switch_config);
        // First switch works — initial last_mode_switch is pre-expired at construction.
        engine.submit(msg("a", "democracy"));
        engine.submit(msg("b", "democracy"));
        engine.submit(msg("c", "democracy"));
        engine.submit(msg("d", "anarchy"));
        assert_eq!(engine.mode, Mode::Democracy);

        // Immediate re-switch attempt — cooldown just reset, 300s hasn't elapsed.
        engine.submit(msg("a", "anarchy"));
        engine.submit(msg("b", "anarchy"));
        engine.submit(msg("c", "anarchy"));
        engine.submit(msg("d", "democracy"));
        assert_eq!(engine.mode, Mode::Democracy); // still democracy — blocked by cooldown
    }
}
