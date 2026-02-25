use rustboyadvance_ng::prelude::GameBoyAdvance;
use serde::{Deserialize, Serialize};

use super::{
    charmap::decode_string,
    decrypt::{attacks_slot, decrypt_block, get_substructure, growth_slot, read_u16},
    Gen3Game,
};

const PARTY_SIZE: usize = 6;
const ENTRY_BYTES: usize = 100;
const NICKNAME_LEN: usize = 10;

// Offsets within the 100-byte party pokemon struct (unencrypted section)
const OFF_PID: usize = 0x00;
const OFF_OT_ID: usize = 0x04;
const OFF_NICKNAME: usize = 0x08;
const OFF_ENCRYPTED: usize = 0x20;
const OFF_STATUS: usize = 0x50;
const OFF_LEVEL: usize = 0x54;
const OFF_CURRENT_HP: usize = 0x56;
const OFF_MAX_HP: usize = 0x58;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyPokemon {
    pub species: u16,
    pub nickname: String,
    pub level: u8,
    pub current_hp: u16,
    pub max_hp: u16,
    pub status: u32,
    pub moves: [u16; 4],
}

impl PartyPokemon {
    pub fn is_fainted(&self) -> bool {
        self.current_hp == 0
    }
}

fn read_u32_le(gba: &mut GameBoyAdvance, addr: u32) -> u32 {
    let b0 = gba.debug_read_8(addr) as u32;
    let b1 = gba.debug_read_8(addr + 1) as u32;
    let b2 = gba.debug_read_8(addr + 2) as u32;
    let b3 = gba.debug_read_8(addr + 3) as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

fn read_u16_le(gba: &mut GameBoyAdvance, addr: u32) -> u16 {
    let lo = gba.debug_read_8(addr) as u16;
    let hi = gba.debug_read_8(addr + 1) as u16;
    lo | (hi << 8)
}

fn read_bytes(gba: &mut GameBoyAdvance, addr: u32, buf: &mut [u8]) {
    for (i, byte) in buf.iter_mut().enumerate() {
        *byte = gba.debug_read_8(addr + i as u32);
    }
}

fn read_party_entry(gba: &mut GameBoyAdvance, base: u32) -> Option<PartyPokemon> {
    let pid = read_u32_le(gba, base + OFF_PID as u32);
    let ot_id = read_u32_le(gba, base + OFF_OT_ID as u32);

    // Empty slot: pid and ot_id both zero
    if pid == 0 && ot_id == 0 {
        return None;
    }

    let mut nickname_raw = [0u8; NICKNAME_LEN];
    read_bytes(gba, base + OFF_NICKNAME as u32, &mut nickname_raw);
    let nickname = decode_string(&nickname_raw);

    let status = read_u32_le(gba, base + OFF_STATUS as u32);
    let level = gba.debug_read_8(base + OFF_LEVEL as u32);
    let current_hp = read_u16_le(gba, base + OFF_CURRENT_HP as u32);
    let max_hp = read_u16_le(gba, base + OFF_MAX_HP as u32);

    let mut encrypted_raw = [0u8; 48];
    read_bytes(gba, base + OFF_ENCRYPTED as u32, &mut encrypted_raw);
    let decrypted = decrypt_block(&encrypted_raw, pid, ot_id);

    let g_slot = growth_slot(pid);
    let a_slot = attacks_slot(pid);

    let growth = get_substructure(&decrypted, g_slot);
    let species = read_u16(growth, 0x00);

    let attacks = get_substructure(&decrypted, a_slot);
    let moves = [
        read_u16(attacks, 0x00),
        read_u16(attacks, 0x02),
        read_u16(attacks, 0x04),
        read_u16(attacks, 0x06),
    ];

    Some(PartyPokemon {
        species,
        nickname,
        level,
        current_hp,
        max_hp,
        status,
        moves,
    })
}

/// Read all Pokémon currently in the party (up to 6).
/// Empty slots and slots beyond the party count are excluded.
pub fn read_party(gba: &mut GameBoyAdvance, game: Gen3Game) -> Vec<PartyPokemon> {
    let (count_addr, party_addr) = game.party_addrs();

    let count = (read_u32_le(gba, count_addr) as usize).min(PARTY_SIZE);

    (0..count)
        .filter_map(|i| {
            let base = party_addr + (i * ENTRY_BYTES) as u32;
            read_party_entry(gba, base)
        })
        .collect()
}
