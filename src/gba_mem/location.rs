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

/// Address of the gSaveBlock1Ptr variable in IWRAM.
/// Dereference this to get the base address of the SaveBlock1 struct in EWRAM.
/// Source: BPEE community linker script (sav1 = 0x03005D8C).
const SAVE_BLOCK_1_PTR: u32 = 0x03005D8C;

/// Offsets within SaveBlock1 (from pokeemerald include/global.h struct SaveBlock1):
///   +0x00: Coords16 pos { s16 x; s16 y; }  — live player tile position
///   +0x04: WarpData location { s8 mapGroup; s8 mapNum; ... }
mod offset {
    pub const PLAYER_X: u32 = 0x00; // pos.x
    pub const PLAYER_Y: u32 = 0x02; // pos.y
    pub const MAP_BANK: u32 = 0x04; // location.mapGroup
    pub const MAP_NUM:  u32 = 0x05; // location.mapNum
}

fn read_u16_le(gba: &mut GameBoyAdvance, addr: u32) -> u16 {
    let lo = gba.debug_read_8(addr) as u16;
    let hi = gba.debug_read_8(addr + 1) as u16;
    lo | (hi << 8)
}

fn read_u32_le(gba: &mut GameBoyAdvance, addr: u32) -> u32 {
    let b0 = gba.debug_read_8(addr) as u32;
    let b1 = gba.debug_read_8(addr + 1) as u32;
    let b2 = gba.debug_read_8(addr + 2) as u32;
    let b3 = gba.debug_read_8(addr + 3) as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

pub fn read_location(gba: &mut GameBoyAdvance) -> PlayerLocation {
    let save1 = read_u32_le(gba, SAVE_BLOCK_1_PTR);
    PlayerLocation {
        map_bank: gba.debug_read_8(save1 + offset::MAP_BANK),
        map_num: gba.debug_read_8(save1 + offset::MAP_NUM),
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
