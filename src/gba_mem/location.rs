use rustboyadvance_ng::prelude::GameBoyAdvance;
use serde::{Deserialize, Serialize};

use super::{read_u16_le, read_u8, save1_base};

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

/// Offsets within SaveBlock1 (from pokeemerald include/global.h struct SaveBlock1):
///   +0x00: Coords16 pos { s16 x; s16 y; }  — live player tile position
///   +0x04: WarpData location { s8 mapGroup; s8 mapNum; ... }
mod offset {
    pub const PLAYER_X: u32 = 0x00; // pos.x
    pub const PLAYER_Y: u32 = 0x02; // pos.y
    pub const MAP_BANK: u32 = 0x04; // location.mapGroup
    pub const MAP_NUM:  u32 = 0x05; // location.mapNum
}

pub fn read_location(gba: &mut GameBoyAdvance) -> PlayerLocation {
    let save1 = save1_base(gba);
    PlayerLocation {
        map_bank: read_u8(gba, save1 + offset::MAP_BANK),
        map_num: read_u8(gba, save1 + offset::MAP_NUM),
        x: read_u16_le(gba, save1 + offset::PLAYER_X),
        y: read_u16_le(gba, save1 + offset::PLAYER_Y),
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
