use rustboyadvance_ng::prelude::GameBoyAdvance;
use serde::{Deserialize, Serialize};

use super::{read_u8, save1_base};

/// Gym badge state read from GBA save flags.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BadgeState {
    /// Bitmask of earned badges: bit 0 = Stone, bit 1 = Knuckle, ..., bit 7 = Rain.
    pub badges: u8,
}

impl BadgeState {
    pub fn has_badge(self, index: u8) -> bool {
        (self.badges >> index) & 1 == 1
    }

    pub fn count(self) -> u32 {
        self.badges.count_ones()
    }
}

/// Offset of `flags[]` within SaveBlock1 (pokeemerald include/global.h:1020).
const FLAGS_ARRAY_OFFSET: u32 = 0x1270;

/// Flag index of FLAG_BADGE01_GET (pokeemerald include/constants/flags.h).
/// Badges 1–8 occupy flag indices 0x867–0x86E consecutively.
const FLAG_BADGE01_GET: u32 = 0x867;

fn read_flag(gba: &mut GameBoyAdvance, save1: u32, flag_id: u32) -> bool {
    let byte_addr = save1 + FLAGS_ARRAY_OFFSET + (flag_id >> 3);
    let bit = (flag_id & 7) as u8;
    (read_u8(gba, byte_addr) >> bit) & 1 == 1
}

pub fn read_badges(gba: &mut GameBoyAdvance) -> BadgeState {
    let save1 = save1_base(gba);
    let mut badges = 0u8;
    for i in 0..8u32 {
        if read_flag(gba, save1, FLAG_BADGE01_GET + i) {
            badges |= 1 << i;
        }
    }
    BadgeState { badges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_state_serializes() {
        let state = BadgeState { badges: 0b00000101 };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"badges\":5"));
    }

    #[test]
    fn test_badge_state_deserializes() {
        let json = r#"{"badges":255}"#;
        let state: BadgeState = serde_json::from_str(json).unwrap();
        assert_eq!(state.badges, 255);
    }

    #[test]
    fn test_has_badge() {
        let state = BadgeState { badges: 0b00000101 };
        assert!(state.has_badge(0));
        assert!(!state.has_badge(1));
        assert!(state.has_badge(2));
    }

    #[test]
    fn test_count() {
        assert_eq!(BadgeState { badges: 0b11111111 }.count(), 8);
        assert_eq!(BadgeState { badges: 0b00000000 }.count(), 0);
        assert_eq!(BadgeState { badges: 0b00000011 }.count(), 2);
    }

    #[test]
    fn test_no_badges_is_zero() {
        let state = BadgeState { badges: 0 };
        for i in 0..8 {
            assert!(!state.has_badge(i));
        }
    }
}
