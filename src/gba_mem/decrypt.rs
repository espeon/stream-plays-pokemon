/// The 24 substructure orderings indexed by (personality_value % 24).
/// Each entry is [G_pos, A_pos, E_pos, M_pos] — the index of each substructure
/// within the four 12-byte slots of the encrypted block.
///
/// Equivalently, ORDER[i] is the sequence of substructure letters for permutation i.
/// We store it as which slot each letter lands in so we can find G quickly.
const SUBSTRUCTURE_ORDER: [[u8; 4]; 24] = [
    // index: [G, A, E, M] slot positions
    // Derived from the GAEM permutation table on Bulbapedia.
    // Each row is the ORDER string, stored as slot indices:
    //   ORDER[i][0] = slot of G, [1] = slot of A, [2] = slot of E, [3] = slot of M
    [0, 1, 2, 3], // 0: GAEM
    [0, 1, 3, 2], // 1: GAME
    [0, 2, 1, 3], // 2: GEAM
    [0, 3, 1, 2], // 3: GEMA
    [0, 2, 3, 1], // 4: GMAE
    [0, 3, 2, 1], // 5: GMEA
    [1, 0, 2, 3], // 6: AGEM
    [1, 0, 3, 2], // 7: AGME
    [2, 0, 1, 3], // 8: AEGM
    [3, 0, 1, 2], // 9: AEMG
    [2, 0, 3, 1], // 10: AMGE
    [3, 0, 2, 1], // 11: AMEG
    [1, 2, 0, 3], // 12: EGAM
    [1, 3, 0, 2], // 13: EGMA
    [2, 1, 0, 3], // 14: EAGM
    [3, 1, 0, 2], // 15: EAMG
    [2, 3, 0, 1], // 16: EMGA
    [3, 2, 0, 1], // 17: EMAG
    [1, 2, 3, 0], // 18: MGAE
    [1, 3, 2, 0], // 19: MGEA
    [2, 1, 3, 0], // 20: MAGE
    [3, 1, 2, 0], // 21: MAEG
    [2, 3, 1, 0], // 22: MEGA
    [3, 2, 1, 0], // 23: MEAG
];

/// Decrypt the 48-byte Gen III pokemon data block.
/// Key = personality_value XOR ot_id, applied as 32-bit words.
/// Returns the decrypted block as 48 bytes.
pub fn decrypt_block(encrypted: &[u8; 48], pid: u32, ot_id: u32) -> [u8; 48] {
    let key = pid ^ ot_id;
    let mut out = [0u8; 48];
    for i in (0..48).step_by(4) {
        let word = u32::from_le_bytes(encrypted[i..i + 4].try_into().unwrap());
        let decrypted = word ^ key;
        out[i..i + 4].copy_from_slice(&decrypted.to_le_bytes());
    }
    out
}

/// Extract a substructure from the decrypted 48-byte block by its slot index (0–3).
/// Each substructure is 12 bytes.
pub fn get_substructure(decrypted: &[u8; 48], slot: u8) -> &[u8] {
    let start = slot as usize * 12;
    &decrypted[start..start + 12]
}

/// Return the slot index of the Growth (G) substructure for the given personality value.
pub fn growth_slot(pid: u32) -> u8 {
    SUBSTRUCTURE_ORDER[(pid % 24) as usize][0]
}

/// Return the slot index of the Attacks (A) substructure for the given personality value.
pub fn attacks_slot(pid: u32) -> u8 {
    SUBSTRUCTURE_ORDER[(pid % 24) as usize][1]
}

/// Read a u16 from a substructure at a given byte offset (little-endian).
pub fn read_u16(sub: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(sub[offset..offset + 2].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypt_with_zero_key() {
        let mut encrypted = [0u8; 48];
        encrypted[0] = 0xAB;
        encrypted[1] = 0xCD;
        encrypted[2] = 0xEF;
        encrypted[3] = 0x01;
        let decrypted = decrypt_block(&encrypted, 0, 0); // key = 0 ^ 0 = 0
        assert_eq!(&decrypted[0..4], &encrypted[0..4]);
    }

    #[test]
    fn test_decrypt_xor_applied() {
        let mut encrypted = [0u8; 48];
        // word at offset 0 = 0x00000001
        encrypted[0] = 0x01;
        // key = 0xDEADBEEF ^ 0x00000000 = 0xDEADBEEF
        let decrypted = decrypt_block(&encrypted, 0xDEADBEEF, 0x00000000);
        let expected = (0x00000001u32 ^ 0xDEADBEEFu32).to_le_bytes();
        assert_eq!(&decrypted[0..4], &expected);
    }

    #[test]
    fn test_decrypt_roundtrip() {
        // Encrypt then decrypt should recover original.
        let original = [0u8; 48].map(|_| rand_byte());
        let pid = 0x12345678u32;
        let ot_id = 0xABCDEF01u32;
        let encrypted = decrypt_block(&original, pid, ot_id); // XOR is its own inverse
        let recovered = decrypt_block(&encrypted, pid, ot_id);
        assert_eq!(original, recovered);
    }

    fn rand_byte() -> u8 {
        42 // deterministic stand-in
    }

    #[test]
    fn test_growth_slot_permutation_0() {
        // pid % 24 == 0 → GAEM, G is at slot 0
        assert_eq!(growth_slot(0), 0);
        assert_eq!(growth_slot(24), 0);
        assert_eq!(growth_slot(48), 0);
    }

    #[test]
    fn test_growth_slot_permutation_6() {
        // pid % 24 == 6 → AGEM, G is at slot 1
        assert_eq!(growth_slot(6), 1);
    }

    #[test]
    fn test_growth_slot_permutation_12() {
        // pid % 24 == 12 → EGAM, G is at slot 2 (E=0, G=1... wait: EGAM = E@0, G@1, A@2, M@3)
        // So G slot = 1
        assert_eq!(growth_slot(12), 1);
    }

    #[test]
    fn test_attacks_slot_permutation_0() {
        // pid % 24 == 0 → GAEM, A is at slot 1
        assert_eq!(attacks_slot(0), 1);
    }

    #[test]
    fn test_get_substructure_slot_0() {
        let mut block = [0u8; 48];
        block[0] = 0xAA;
        block[11] = 0xBB;
        let sub = get_substructure(&block, 0);
        assert_eq!(sub[0], 0xAA);
        assert_eq!(sub[11], 0xBB);
        assert_eq!(sub.len(), 12);
    }

    #[test]
    fn test_get_substructure_slot_3() {
        let mut block = [0u8; 48];
        block[36] = 0xCC;
        let sub = get_substructure(&block, 3);
        assert_eq!(sub[0], 0xCC);
    }

    #[test]
    fn test_all_24_growth_slots_valid() {
        for i in 0u32..24 {
            let slot = growth_slot(i);
            assert!(slot < 4, "growth_slot({i}) = {slot} out of range");
        }
    }

    #[test]
    fn test_substructure_order_table_has_unique_slots_per_row() {
        for (i, row) in SUBSTRUCTURE_ORDER.iter().enumerate() {
            let mut seen = [false; 4];
            for &slot in row {
                assert!(slot < 4, "row {i} has slot {slot} >= 4");
                assert!(!seen[slot as usize], "row {i} has duplicate slot {slot}");
                seen[slot as usize] = true;
            }
        }
    }
}
