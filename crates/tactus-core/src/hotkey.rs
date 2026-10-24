use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlobalAction {
    PlayPause,
    Stop,
    Next,
    Previous,
    SeekForward,
    SeekBack,
    VolumeUp,
    VolumeDown,
    Like,
    Dislike,
    Wave,
    NowPlaying,
    ShowWindow,
    Voice,
}

impl GlobalAction {
    pub const ALL: [GlobalAction; 14] = [
        GlobalAction::PlayPause,
        GlobalAction::Stop,
        GlobalAction::Next,
        GlobalAction::Previous,
        GlobalAction::SeekForward,
        GlobalAction::SeekBack,
        GlobalAction::VolumeUp,
        GlobalAction::VolumeDown,
        GlobalAction::Like,
        GlobalAction::Dislike,
        GlobalAction::Wave,
        GlobalAction::NowPlaying,
        GlobalAction::ShowWindow,
        GlobalAction::Voice,
    ];

    pub fn default_key(self) -> &'static str {
        match self {
            GlobalAction::PlayPause => "Ctrl+Alt+Shift+P",
            GlobalAction::Stop => "Ctrl+Alt+Shift+S",
            GlobalAction::Next => "Ctrl+Alt+Shift+Right",
            GlobalAction::Previous => "Ctrl+Alt+Shift+Left",
            GlobalAction::SeekForward => "Ctrl+Alt+Shift+PageDown",
            GlobalAction::SeekBack => "Ctrl+Alt+Shift+PageUp",
            GlobalAction::VolumeUp => "Ctrl+Alt+Shift+Up",
            GlobalAction::VolumeDown => "Ctrl+Alt+Shift+Down",
            GlobalAction::Like => "Ctrl+Alt+Shift+L",
            GlobalAction::Dislike => "Ctrl+Alt+Shift+D",
            GlobalAction::Wave => "Ctrl+Alt+Shift+M",
            GlobalAction::NowPlaying => "Ctrl+Alt+Shift+I",
            GlobalAction::ShowWindow => "Ctrl+Alt+Shift+W",
            GlobalAction::Voice => "Ctrl+Alt+Shift+Space",
        }
    }

    pub fn repeats(self) -> bool {
        matches!(
            self,
            GlobalAction::VolumeUp | GlobalAction::VolumeDown | GlobalAction::SeekForward | GlobalAction::SeekBack
        )
    }

    pub fn key_in(self, bindings: &BTreeMap<GlobalAction, String>) -> Option<String> {
        let key = bindings.get(&self).map_or(self.default_key(), String::as_str);
        (!key.is_empty()).then(|| key.to_owned())
    }

    pub fn defaults() -> BTreeMap<GlobalAction, String> {
        Self::ALL.into_iter().map(|action| (action, action.default_key().to_owned())).collect()
    }
}

const KEYS: &[(&str, u32)] = &[
    ("Space", 0x20),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("End", 0x23),
    ("Home", 0x24),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("Backspace", 0x08),
    ("Enter", 0x0D),
    ("Pause", 0x13),
    ("Plus", 0xBB),
    ("Comma", 0xBC),
    ("Minus", 0xBD),
    ("Period", 0xBE),
    ("Slash", 0xBF),
    ("VolumeMute", 0xAD),
    ("VolumeDown", 0xAE),
    ("VolumeUp", 0xAF),
    ("MediaNext", 0xB0),
    ("MediaPrevious", 0xB1),
    ("MediaStop", 0xB2),
    ("MediaPlayPause", 0xB3),
];

const MODIFIERS: [(&str, u32); 4] = [("Ctrl", MOD_CONTROL), ("Alt", MOD_ALT), ("Shift", MOD_SHIFT), ("Win", MOD_WIN)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}

fn key_code(name: &str) -> Option<u32> {
    let upper = name.to_ascii_uppercase();
    if upper.len() == 1 && upper.chars().all(|c| c.is_ascii_alphanumeric()) {
        return upper.bytes().next().map(u32::from);
    }
    if let Some(number) = upper.strip_prefix('F').and_then(|number| number.parse::<u32>().ok()) {
        return (1..=24).contains(&number).then_some(0x6F + number);
    }
    if let Some(digit) = upper.strip_prefix("NUM").and_then(|digit| digit.parse::<u32>().ok()) {
        return (digit <= 9).then_some(0x60 + digit);
    }
    KEYS.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, code)| *code)
}

fn key_name(code: u32) -> String {
    match code {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(code).map(String::from).unwrap_or_default(),
        0x60..=0x69 => format!("Num{}", code - 0x60),
        0x70..=0x87 => format!("F{}", code - 0x6F),
        _ => KEYS.iter().find(|(_, key)| *key == code).map(|(name, _)| (*name).to_owned()).unwrap_or_default(),
    }
}

impl Hotkey {
    pub fn parse(text: &str) -> Option<Hotkey> {
        let mut modifiers = 0;
        let mut key = None;
        for part in text.split('+').map(str::trim).filter(|part| !part.is_empty()) {
            let modifier = match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => MOD_CONTROL,
                "alt" => MOD_ALT,
                "shift" => MOD_SHIFT,
                "win" | "windows" => MOD_WIN,
                _ => 0,
            };
            if modifier != 0 {
                modifiers |= modifier;
            } else if key.replace(key_code(part)?).is_some() {
                return None;
            }
        }
        let key = key?;
        let standalone = matches!(key, 0x70..=0x87 | 0xAD..=0xB3 | 0x13);
        (modifiers != 0 || standalone).then_some(Hotkey { modifiers, key })
    }
}

impl std::fmt::Display for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, flag) in MODIFIERS {
            if self.modifiers & flag != 0 {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&key_name(self.key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_hotkeys() {
        let hotkey = Hotkey::parse("ctrl + alt+shift+p").unwrap();
        assert_eq!(hotkey.modifiers, MOD_CONTROL | MOD_ALT | MOD_SHIFT);
        assert_eq!(hotkey.key, u32::from(b'P'));
        assert_eq!(hotkey.to_string(), "Ctrl+Alt+Shift+P");
        assert_eq!(Hotkey::parse("Win+F5").unwrap().to_string(), "Win+F5");
        assert_eq!(Hotkey::parse("Alt+PageDown").unwrap().to_string(), "Alt+PageDown");
        assert_eq!(Hotkey::parse("MediaPlayPause").unwrap().to_string(), "MediaPlayPause");
    }

    #[test]
    fn rejects_unsafe_or_invalid_hotkeys() {
        assert_eq!(Hotkey::parse("P"), None);
        assert_eq!(Hotkey::parse("Ctrl+P+Q"), None);
        assert_eq!(Hotkey::parse("Ctrl+Nope"), None);
        assert_eq!(Hotkey::parse(""), None);
    }

    #[test]
    fn only_continuous_actions_repeat_while_held() {
        assert!(GlobalAction::VolumeUp.repeats() && GlobalAction::SeekBack.repeats());
        assert!(!GlobalAction::PlayPause.repeats() && !GlobalAction::Like.repeats());
    }

    #[test]
    fn every_default_parses() {
        for action in GlobalAction::ALL {
            assert!(Hotkey::parse(action.default_key()).is_some(), "{action:?}");
        }
    }

    fn codes() -> Vec<u32> {
        (0x30..=0x39)
            .chain(0x41..=0x5A)
            .chain(0x60..=0x69)
            .chain(0x70..=0x87)
            .chain(KEYS.iter().map(|(_, code)| *code))
            .collect()
    }

    proptest::proptest! {
        #[test]
        fn formatted_hotkeys_parse_back(modifiers in 1u32..16, index in 0usize..1000) {
            let codes = codes();
            let hotkey = Hotkey { modifiers, key: codes[index % codes.len()] };
            proptest::prop_assert_eq!(Hotkey::parse(&hotkey.to_string()), Some(hotkey));
            proptest::prop_assert_eq!(Hotkey::parse(&hotkey.to_string().to_lowercase()), Some(hotkey));
        }

        #[test]
        fn parsing_never_panics(text in "\\PC{0,40}") {
            if let Some(hotkey) = Hotkey::parse(&text) {
                proptest::prop_assert_eq!(Hotkey::parse(&hotkey.to_string()), Some(hotkey));
            }
        }
    }
}
