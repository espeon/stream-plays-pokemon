use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::input::types::{ChatMessage, GbaButton, ParsedInput};

pub struct AnarchyQueue {
    queue: VecDeque<(GbaButton, String)>,
    last_input: HashMap<String, Instant>,
    last_start: Option<Instant>,
    rate_limit: Duration,
    start_throttle: Duration,
    capacity: usize,
}

impl AnarchyQueue {
    pub fn new(rate_limit_ms: u64, start_throttle_secs: u64, capacity: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            last_input: HashMap::new(),
            last_start: None,
            rate_limit: Duration::from_millis(rate_limit_ms),
            start_throttle: Duration::from_secs(start_throttle_secs),
            capacity,
        }
    }

    pub fn submit(&mut self, msg: &ChatMessage, input: &ParsedInput) {
        let buttons = input.expand();
        if buttons.is_empty() {
            return;
        }

        let now = Instant::now();

        // Per-user rate limit
        if let Some(&last) = self.last_input.get(&msg.user) {
            if now.duration_since(last) < self.rate_limit {
                return;
            }
        }

        // Start button global throttle
        if buttons.iter().any(|b| *b == GbaButton::Start) {
            if let Some(last) = self.last_start {
                if now.duration_since(last) < self.start_throttle {
                    return;
                }
            }
            self.last_start = Some(now);
        }

        self.last_input.insert(msg.user.clone(), now);

        for button in buttons {
            if self.queue.len() >= self.capacity {
                self.queue.pop_front();
            }
            self.queue.push_back((button, msg.user.clone()));
        }
    }

    pub fn pop(&mut self) -> Option<(GbaButton, String)> {
        self.queue.pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(user: &str) -> ChatMessage {
        ChatMessage { user: user.to_string(), text: String::new(), ts: 0 }
    }

    fn btn(b: GbaButton) -> ParsedInput {
        ParsedInput::Button(b)
    }

    #[test]
    fn test_basic_enqueue_and_pop() {
        let mut q = AnarchyQueue::new(0, 5, 32);
        q.submit(&msg("alice"), &btn(GbaButton::A));
        assert_eq!(q.pop(), Some((GbaButton::A, "alice".to_string())));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn test_per_user_rate_limit_blocks_fast_inputs() {
        let mut q = AnarchyQueue::new(200, 5, 32);
        q.submit(&msg("alice"), &btn(GbaButton::A));
        q.submit(&msg("alice"), &btn(GbaButton::B)); // within 200ms — blocked
        assert_eq!(q.len(), 1);
        assert_eq!(q.pop().unwrap().0, GbaButton::A);
    }

    #[test]
    fn test_different_users_not_rate_limited_by_each_other() {
        let mut q = AnarchyQueue::new(200, 5, 32);
        q.submit(&msg("alice"), &btn(GbaButton::A));
        q.submit(&msg("bob"), &btn(GbaButton::B));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn test_capacity_drops_oldest() {
        let mut q = AnarchyQueue::new(0, 5, 3);
        q.submit(&msg("a"), &btn(GbaButton::A));
        q.submit(&msg("b"), &btn(GbaButton::B));
        q.submit(&msg("c"), &btn(GbaButton::Up));
        q.submit(&msg("d"), &btn(GbaButton::Down)); // drops A
        assert_eq!(q.len(), 3);
        assert_eq!(q.pop().unwrap().0, GbaButton::B);
    }

    #[test]
    fn test_start_throttle_blocks_rapid_start() {
        let mut q = AnarchyQueue::new(0, 5, 32);
        q.submit(&msg("alice"), &btn(GbaButton::Start));
        q.submit(&msg("bob"), &btn(GbaButton::Start)); // within 5s throttle — blocked
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn test_compound_input_enqueues_multiple() {
        let mut q = AnarchyQueue::new(0, 5, 32);
        q.submit(&msg("alice"), &ParsedInput::Compound(GbaButton::Right, 3));
        assert_eq!(q.len(), 3);
        assert_eq!(q.pop().unwrap().0, GbaButton::Right);
        assert_eq!(q.pop().unwrap().0, GbaButton::Right);
        assert_eq!(q.pop().unwrap().0, GbaButton::Right);
    }

    #[test]
    fn test_wait_and_votes_do_nothing() {
        let mut q = AnarchyQueue::new(0, 5, 32);
        q.submit(&msg("alice"), &ParsedInput::Wait);
        q.submit(&msg("alice"), &ParsedInput::VoteAnarchy);
        assert_eq!(q.len(), 0);
    }
}
