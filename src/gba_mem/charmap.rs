/// International (non-Japanese) Gen III character encoding → UTF-8.
/// Returns `None` for the string terminator (0xFF) and unknown/control bytes.
pub fn decode_char(byte: u8) -> Option<char> {
    match byte {
        0x00 => Some('À'),
        0x01 => Some('Á'),
        0x02 => Some('Â'),
        0x03 => Some('Ç'),
        0x04 => Some('È'),
        0x05 => Some('É'),
        0x06 => Some('Ê'),
        0x07 => Some('Ë'),
        0x08 => Some('Ì'),
        0x0A => Some('Î'),
        0x0B => Some('Ï'),
        0x0C => Some('Ò'),
        0x0D => Some('Ó'),
        0x0E => Some('Ô'),
        0x10 => Some('Œ'),
        0x11 => Some('Ù'),
        0x12 => Some('Ú'),
        0x13 => Some('Û'),
        0x14 => Some('Ñ'),
        0x15 => Some('ß'),
        0x16 => Some('à'),
        0x17 => Some('á'),
        0x18 => Some('ç'),
        0x19 => Some('è'),
        0x1A => Some('é'),
        0x1B => Some('ê'),
        0x1C => Some('ë'),
        0x1D => Some('ì'),
        0x20 => Some('î'),
        0x21 => Some('ï'),
        0x22 => Some('ò'),
        0x23 => Some('ó'),
        0x24 => Some('ô'),
        0x25 => Some('œ'),
        0x26 => Some('ù'),
        0x27 => Some('ú'),
        0x28 => Some('û'),
        0x29 => Some('ñ'),
        0x2A => Some('º'),
        0x2B => Some('ª'),
        0x2D => Some('&'),
        0x2E => Some('+'),
        0x34 => Some('℃'), // "Lv" — no clean single char, use placeholder
        0x35 => Some('='),
        0x36 => Some(';'),
        0x46 => Some('¿'),
        0x47 => Some('¡'),
        0x4D => Some('Í'),
        0x4E => Some('%'),
        0x4F => Some('('),
        0x50 => Some(')'),
        0xA1 => Some('0'),
        0xA2 => Some('1'),
        0xA3 => Some('2'),
        0xA4 => Some('3'),
        0xA5 => Some('4'),
        0xA6 => Some('5'),
        0xA7 => Some('6'),
        0xA8 => Some('7'),
        0xA9 => Some('8'),
        0xAA => Some('9'),
        0xAB => Some('!'),
        0xAC => Some('?'),
        0xAD => Some('.'),
        0xAE => Some('-'),
        0xB5 => Some('♂'),
        0xB6 => Some('♀'),
        0xB7 => Some('$'),
        0xB8 => Some(','),
        0xB9 => Some('×'),
        0xBA => Some('/'),
        0xBB => Some('A'),
        0xBC => Some('B'),
        0xBD => Some('C'),
        0xBE => Some('D'),
        0xBF => Some('E'),
        0xC0 => Some('F'),
        0xC1 => Some('G'),
        0xC2 => Some('H'),
        0xC3 => Some('I'),
        0xC4 => Some('J'),
        0xC5 => Some('K'),
        0xC6 => Some('L'),
        0xC7 => Some('M'),
        0xC8 => Some('N'),
        0xC9 => Some('O'),
        0xCA => Some('P'),
        0xCB => Some('Q'),
        0xCC => Some('R'),
        0xCD => Some('S'),
        0xCE => Some('T'),
        0xCF => Some('U'),
        0xD0 => Some('V'),
        0xD1 => Some('W'),
        0xD2 => Some('X'),
        0xD3 => Some('Y'),
        0xD4 => Some('Z'),
        0xD5 => Some('a'),
        0xD6 => Some('b'),
        0xD7 => Some('c'),
        0xD8 => Some('d'),
        0xD9 => Some('e'),
        0xDA => Some('f'),
        0xDB => Some('g'),
        0xDC => Some('h'),
        0xDD => Some('i'),
        0xDE => Some('j'),
        0xDF => Some('k'),
        0xE0 => Some('l'),
        0xE1 => Some('m'),
        0xE2 => Some('n'),
        0xE3 => Some('o'),
        0xE4 => Some('p'),
        0xE5 => Some('q'),
        0xE6 => Some('r'),
        0xE7 => Some('s'),
        0xE8 => Some('t'),
        0xE9 => Some('u'),
        0xEA => Some('v'),
        0xEB => Some('w'),
        0xEC => Some('x'),
        0xED => Some('y'),
        0xEE => Some('z'),
        0xEF => Some('►'),
        0xF0 => Some(':'),
        0xF1 => Some('Ä'),
        0xF2 => Some('Ö'),
        0xF3 => Some('Ü'),
        0xF4 => Some('ä'),
        0xF5 => Some('ö'),
        0xF6 => Some('ü'),
        0xFF => None, // string terminator
        _ => None,
    }
}

/// Decode a Gen III encoded byte slice into a UTF-8 String.
/// Stops at the 0xFF terminator or end of slice.
pub fn decode_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0xFF)
        .filter_map(|&b| decode_char(b))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_uppercase_a_to_z() {
        for (i, expected) in ('A'..='Z').enumerate() {
            let byte = 0xBBu8 + i as u8;
            assert_eq!(decode_char(byte), Some(expected), "byte 0x{byte:02X}");
        }
    }

    #[test]
    fn test_decode_lowercase_a_to_z() {
        for (i, expected) in ('a'..='z').enumerate() {
            let byte = 0xD5u8 + i as u8;
            assert_eq!(decode_char(byte), Some(expected), "byte 0x{byte:02X}");
        }
    }

    #[test]
    fn test_decode_digits_0_to_9() {
        for (i, expected) in ('0'..='9').enumerate() {
            let byte = 0xA1u8 + i as u8;
            assert_eq!(decode_char(byte), Some(expected), "byte 0x{byte:02X}");
        }
    }

    #[test]
    fn test_terminator_returns_none() {
        assert_eq!(decode_char(0xFF), None);
    }

    #[test]
    fn test_decode_string_stops_at_terminator() {
        // "Hi" followed by terminator and more data
        let bytes = [0xDC, 0xDD, 0xFF, 0xBB, 0xBC];
        assert_eq!(decode_string(&bytes), "hi");
    }

    #[test]
    fn test_decode_string_full_without_terminator() {
        // "ABC"
        let bytes = [0xBB, 0xBC, 0xBD];
        assert_eq!(decode_string(&bytes), "ABC");
    }

    #[test]
    fn test_decode_string_empty_on_immediate_terminator() {
        assert_eq!(decode_string(&[0xFF]), "");
    }

    #[test]
    fn test_decode_gender_symbols() {
        assert_eq!(decode_char(0xB5), Some('♂'));
        assert_eq!(decode_char(0xB6), Some('♀'));
    }
}
