pub mod badges;
pub mod charmap;
pub mod decrypt;
pub mod location;
pub mod party;

use rustboyadvance_ng::prelude::GameBoyAdvance;

/// Pointer to SaveBlock1 in IWRAM; dereference to get the EWRAM base address.
/// Source: BPEE community linker script (sav1 = 0x03005D8C).
pub const SAVE_BLOCK_1_PTR: u32 = 0x03005D8C;

pub fn read_u8(gba: &mut GameBoyAdvance, addr: u32) -> u8 {
    gba.debug_read_8(addr)
}

pub fn read_u16_le(gba: &mut GameBoyAdvance, addr: u32) -> u16 {
    let lo = gba.debug_read_8(addr) as u16;
    let hi = gba.debug_read_8(addr + 1) as u16;
    lo | (hi << 8)
}

pub fn read_u32_le(gba: &mut GameBoyAdvance, addr: u32) -> u32 {
    let b0 = gba.debug_read_8(addr) as u32;
    let b1 = gba.debug_read_8(addr + 1) as u32;
    let b2 = gba.debug_read_8(addr + 2) as u32;
    let b3 = gba.debug_read_8(addr + 3) as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

/// Dereference SAVE_BLOCK_1_PTR to get the base address of SaveBlock1 in EWRAM.
pub fn save1_base(gba: &mut GameBoyAdvance) -> u32 {
    read_u32_le(gba, SAVE_BLOCK_1_PTR)
}

/// Identifies a Gen III Pokémon game by its ROM header game code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gen3Game {
    Emerald,
    Ruby,
    Sapphire,
    FireRed,
    LeafGreen,
}

impl Gen3Game {
    /// Detect the game from the ROM header game code returned by `gba.get_game_code()`.
    /// Returns `None` if the code is not a recognized Gen III game.
    pub fn detect(game_code: &str) -> Option<Self> {
        // Codes are 4 chars: first 3 identify game, 4th is region (E=English, etc.)
        match game_code.get(..3)? {
            "BPE" => Some(Self::Emerald),
            "AXV" => Some(Self::Ruby),
            "AXP" => Some(Self::Sapphire),
            "BPR" => Some(Self::FireRed),
            "BPG" => Some(Self::LeafGreen),
            _ => None,
        }
    }

    /// Returns (party_count_addr, party_array_addr) for this game.
    /// Party count is stored as u32 at 4 bytes before the party array.
    pub fn party_addrs(self) -> (u32, u32) {
        let party = match self {
            Self::Emerald => 0x020244EC,
            Self::Ruby => 0x03004360,
            Self::Sapphire => 0x03004360,
            Self::FireRed => 0x02024284,
            Self::LeafGreen => 0x02024284,
        };
        (party - 4, party)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_emerald() {
        assert_eq!(Gen3Game::detect("BPEE"), Some(Gen3Game::Emerald));
        assert_eq!(Gen3Game::detect("BPEF"), Some(Gen3Game::Emerald));
    }

    #[test]
    fn test_detect_ruby() {
        assert_eq!(Gen3Game::detect("AXVE"), Some(Gen3Game::Ruby));
    }

    #[test]
    fn test_detect_sapphire() {
        assert_eq!(Gen3Game::detect("AXPE"), Some(Gen3Game::Sapphire));
    }

    #[test]
    fn test_detect_firered() {
        assert_eq!(Gen3Game::detect("BPRE"), Some(Gen3Game::FireRed));
    }

    #[test]
    fn test_detect_leafgreen() {
        assert_eq!(Gen3Game::detect("BPGE"), Some(Gen3Game::LeafGreen));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(Gen3Game::detect("XXXX"), None);
        assert_eq!(Gen3Game::detect(""), None);
    }

    #[test]
    fn test_party_addrs_emerald() {
        let (count, party) = Gen3Game::Emerald.party_addrs();
        assert_eq!(party, 0x020244EC);
        assert_eq!(count, party - 4);
    }

    #[test]
    fn test_party_addrs_ruby_sapphire() {
        let (_, ruby) = Gen3Game::Ruby.party_addrs();
        let (_, sapphire) = Gen3Game::Sapphire.party_addrs();
        assert_eq!(ruby, 0x03004360);
        assert_eq!(sapphire, 0x03004360);
    }
}
