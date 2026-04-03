#![allow(dead_code, unused_imports)]

pub mod chat;
pub mod config;
pub mod emulator;
pub mod error;
pub mod gba_mem;
pub mod input;
pub mod save;
pub mod server;
pub mod types;
pub mod vote;

// view count tracker
pub struct ViewerCountTracker {
    count: u32,
}

impl ViewerCountTracker {
    pub fn new() -> Self {
        Self { count: 0 }
    }

    pub fn update(&mut self, new_count: u32) {
        if new_count != self.count {
            tracing::info!("viewer count updated: {new_count}");
            self.count = new_count;
        }
    }
}
