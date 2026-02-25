use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GbaButton {
    A,
    B,
    Up,
    Down,
    Left,
    Right,
    Start,
    Select,
    L,
    R,
}

impl GbaButton {
    pub fn as_str(self) -> &'static str {
        match self {
            GbaButton::A => "a",
            GbaButton::B => "b",
            GbaButton::Up => "up",
            GbaButton::Down => "down",
            GbaButton::Left => "left",
            GbaButton::Right => "right",
            GbaButton::Start => "start",
            GbaButton::Select => "select",
            GbaButton::L => "l",
            GbaButton::R => "r",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedInput {
    Button(GbaButton),
    Compound(GbaButton, u8),
    Wait,
    VoteAnarchy,
    VoleDemocracy,
}

impl ParsedInput {
    pub fn expand(&self) -> Vec<GbaButton> {
        match self {
            ParsedInput::Button(btn) => vec![*btn],
            ParsedInput::Compound(btn, count) => vec![*btn; *count as usize],
            ParsedInput::Wait => vec![],
            ParsedInput::VoteAnarchy | ParsedInput::VoleDemocracy => vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub user: String,
    pub text: String,
    pub ts: i64,
}
