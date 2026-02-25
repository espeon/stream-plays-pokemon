use rustboyadvance_ng::prelude::GameBoyAdvance;
use serde::{Deserialize, Serialize};

/// Player location read from GBA memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerLocation {
    pub map_bank: u8,
    pub map_num: u8,
    /// Player tile X within the current map
    pub x: u16,
    /// Player tile Y within the current map
    pub y: u16,
}

/// Emerald RAM offsets for player location.
/// Source: pokeemerald decomp / community memory maps.
mod addr {
    pub const PLAYER_X: u32 = 0x02025A08; // u16
    pub const PLAYER_Y: u32 = 0x02025A0A; // u16
    pub const MAP_BANK: u32 = 0x02025A0C; // u8
    pub const MAP_NUM: u32 = 0x02025A0D;  // u8
}

fn read_u16_le(gba: &mut GameBoyAdvance, addr: u32) -> u16 {
    let lo = gba.debug_read_8(addr) as u16;
    let hi = gba.debug_read_8(addr + 1) as u16;
    lo | (hi << 8)
}

pub fn read_location(gba: &mut GameBoyAdvance) -> PlayerLocation {
    PlayerLocation {
        x: read_u16_le(gba, addr::PLAYER_X),
        y: read_u16_le(gba, addr::PLAYER_Y),
        map_bank: gba.debug_read_8(addr::MAP_BANK),
        map_num: gba.debug_read_8(addr::MAP_NUM),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_location_serializes() {
        let loc = PlayerLocation {
            map_bank: 0,
            map_num: 0,
            x: 5,
            y: 7,
        };
        let json = serde_json::to_string(&loc).unwrap();
        assert!(json.contains("\"map_bank\":0"));
        assert!(json.contains("\"map_num\":0"));
        assert!(json.contains("\"x\":5"));
        assert!(json.contains("\"y\":7"));
    }

    #[test]
    fn test_player_location_deserializes() {
        let json = r#"{"map_bank":2,"map_num":14,"x":10,"y":3}"#;
        let loc: PlayerLocation = serde_json::from_str(json).unwrap();
        assert_eq!(loc.map_bank, 2);
        assert_eq!(loc.map_num, 14);
        assert_eq!(loc.x, 10);
        assert_eq!(loc.y, 3);
    }

    #[test]
    fn test_player_location_equality() {
        let a = PlayerLocation { map_bank: 1, map_num: 2, x: 0, y: 0 };
        let b = PlayerLocation { map_bank: 1, map_num: 2, x: 0, y: 0 };
        let c = PlayerLocation { map_bank: 1, map_num: 3, x: 0, y: 0 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
