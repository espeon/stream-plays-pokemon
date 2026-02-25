use tracing::info;

use super::types::{GbaButton, ParsedInput};

const MAX_COMPOUND_REPEAT: u8 = 128;

pub fn parse_chat_message(text: &str) -> Option<ParsedInput> {
    let text = text.trim().to_lowercase();

    match text.as_str() {
        "a" => return Some(ParsedInput::Button(GbaButton::A)),
        "b" => return Some(ParsedInput::Button(GbaButton::B)),
        "up" => return Some(ParsedInput::Button(GbaButton::Up)),
        "down" => return Some(ParsedInput::Button(GbaButton::Down)),
        "left" => return Some(ParsedInput::Button(GbaButton::Left)),
        "right" => return Some(ParsedInput::Button(GbaButton::Right)),
        "start" => return Some(ParsedInput::Button(GbaButton::Start)),
        "select" => return Some(ParsedInput::Button(GbaButton::Select)),
        "l" => return Some(ParsedInput::Button(GbaButton::L)),
        "r" => return Some(ParsedInput::Button(GbaButton::R)),
        "wait" => return Some(ParsedInput::Wait),
        "anarchy" => return Some(ParsedInput::VoteAnarchy),
        "democracy" => return Some(ParsedInput::VoleDemocracy),
        _ => {}
    }

    parse_compound(&text)
}

fn parse_compound(text: &str) -> Option<ParsedInput> {
  if text.len() < 2 {
        return None;
    }

    // get number at end
    // can be up to MAX_COMPOUND_REPEAT which is 3 digits, but we'll just parse until we hit a non-digit
    let mut repeat_str = String::new();
    for c in text.chars().rev() {
        if c.is_digit(10) {
            repeat_str.insert(0, c);
        } else {
            break;
        }
    }
    if repeat_str.is_empty() {
        return None;
    }
    let repeat: u8 = repeat_str.parse().ok()?;
    if repeat == 0 || repeat > MAX_COMPOUND_REPEAT {
        return None;
    }

    let button_str = &text[..text.len() - repeat_str.len()];
    let button = match button_str {
        "a" => GbaButton::A,
        "b" => GbaButton::B,
        "up" => GbaButton::Up,
        "down" => GbaButton::Down,
        "left" => GbaButton::Left,
        "right" => GbaButton::Right,
        "start" => GbaButton::Start,
        "select" => GbaButton::Select,
        "l" => GbaButton::L,
        "r" => GbaButton::R,
        _ => return None,
    };

    Some(ParsedInput::Compound(button, repeat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_basic_buttons() {
        assert_eq!(parse_chat_message("a"), Some(ParsedInput::Button(GbaButton::A)));
        assert_eq!(parse_chat_message("b"), Some(ParsedInput::Button(GbaButton::B)));
        assert_eq!(parse_chat_message("up"), Some(ParsedInput::Button(GbaButton::Up)));
        assert_eq!(parse_chat_message("down"), Some(ParsedInput::Button(GbaButton::Down)));
        assert_eq!(parse_chat_message("left"), Some(ParsedInput::Button(GbaButton::Left)));
        assert_eq!(parse_chat_message("right"), Some(ParsedInput::Button(GbaButton::Right)));
        assert_eq!(parse_chat_message("start"), Some(ParsedInput::Button(GbaButton::Start)));
        assert_eq!(parse_chat_message("select"), Some(ParsedInput::Button(GbaButton::Select)));
        assert_eq!(parse_chat_message("l"), Some(ParsedInput::Button(GbaButton::L)));
        assert_eq!(parse_chat_message("r"), Some(ParsedInput::Button(GbaButton::R)));
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(parse_chat_message("A"), Some(ParsedInput::Button(GbaButton::A)));
        assert_eq!(parse_chat_message("UP"), Some(ParsedInput::Button(GbaButton::Up)));
        assert_eq!(parse_chat_message("Right"), Some(ParsedInput::Button(GbaButton::Right)));
        assert_eq!(parse_chat_message("START"), Some(ParsedInput::Button(GbaButton::Start)));
        assert_eq!(parse_chat_message("DEMOCRACY"), Some(ParsedInput::VoleDemocracy));
    }

    #[test]
    fn test_parses_compound_inputs() {
        assert_eq!(parse_chat_message("right3"), Some(ParsedInput::Compound(GbaButton::Right, 3)));
        assert_eq!(parse_chat_message("a2"), Some(ParsedInput::Compound(GbaButton::A, 2)));
        assert_eq!(parse_chat_message("up9"), Some(ParsedInput::Compound(GbaButton::Up, 9)));
        assert_eq!(parse_chat_message("down5"), Some(ParsedInput::Compound(GbaButton::Down, 5)));
    }

    #[test]
    fn test_parses_mode_votes_and_wait() {
        assert_eq!(parse_chat_message("wait"), Some(ParsedInput::Wait));
        assert_eq!(parse_chat_message("anarchy"), Some(ParsedInput::VoteAnarchy));
        assert_eq!(parse_chat_message("democracy"), Some(ParsedInput::VoleDemocracy));
    }

    #[test]
    fn test_rejects_invalid_inputs() {
        assert_eq!(parse_chat_message("hello world"), None);
        assert_eq!(parse_chat_message(""), None);
        assert_eq!(parse_chat_message("   "), None);
        assert_eq!(parse_chat_message("xyz"), None);
        assert_eq!(parse_chat_message("right10"), None);
        assert_eq!(parse_chat_message("a0"), None);
        assert_eq!(parse_chat_message("a1"), None);
        assert_eq!(parse_chat_message("notabutton3"), None);
    }

    #[test]
    fn test_compound_repeat_cap_at_9() {
        assert_eq!(parse_chat_message("right9"), Some(ParsedInput::Compound(GbaButton::Right, 9)));
        assert_eq!(parse_chat_message("right10"), None);
    }

    #[test]
    fn test_trims_whitespace() {
        assert_eq!(parse_chat_message("  a  "), Some(ParsedInput::Button(GbaButton::A)));
        assert_eq!(parse_chat_message("\tup\n"), Some(ParsedInput::Button(GbaButton::Up)));
    }

    #[test]
    fn test_expand_button() {
        let input = ParsedInput::Button(GbaButton::A);
        assert_eq!(input.expand(), vec![GbaButton::A]);
    }

    #[test]
    fn test_expand_compound() {
        let input = ParsedInput::Compound(GbaButton::Right, 3);
        assert_eq!(input.expand(), vec![GbaButton::Right, GbaButton::Right, GbaButton::Right]);
    }

    #[test]
    fn test_expand_wait_and_votes_are_empty() {
        assert_eq!(ParsedInput::Wait.expand(), vec![]);
        assert_eq!(ParsedInput::VoteAnarchy.expand(), vec![]);
        assert_eq!(ParsedInput::VoleDemocracy.expand(), vec![]);
    }
}
