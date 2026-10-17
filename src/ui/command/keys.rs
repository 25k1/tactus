use super::*;
use std::collections::BTreeMap;

const MODIFIERS: [&str; 3] = ["Ctrl", "Alt", "Shift"];
const NAMED: [(&str, &str); 20] = [
    ("left", "Left"),
    ("right", "Right"),
    ("up", "Up"),
    ("down", "Down"),
    ("home", "Home"),
    ("end", "End"),
    ("pageup", "PageUp"),
    ("pgup", "PageUp"),
    ("pagedown", "PageDown"),
    ("pgdn", "PageDown"),
    ("insert", "Insert"),
    ("delete", "Delete"),
    ("back", "Back"),
    ("backspace", "Back"),
    ("space", "Space"),
    ("tab", "Tab"),
    ("enter", "Enter"),
    ("return", "Enter"),
    ("escape", "Escape"),
    ("esc", "Escape"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Command(Command),
    Section(Section),
}

impl Binding {
    pub fn all() -> Vec<Binding> {
        Command::ALL
            .into_iter()
            .filter(|command| !matches!(command, Command::ToggleWindow | Command::ShowWindow))
            .map(Binding::Command)
            .chain(Section::ALL.into_iter().map(Binding::Section))
            .collect()
    }

    pub fn name(self) -> String {
        match self {
            Binding::Command(command) => format!("{command:?}"),
            Binding::Section(section) => format!("Go{section:?}"),
        }
    }

    pub fn label(self) -> String {
        match self {
            Binding::Command(command) => plain(&command.label()),
            Binding::Section(section) => section.label(),
        }
    }

    pub fn default_key(self) -> Option<&'static str> {
        match self {
            Binding::Command(command) => command.default_key(),
            Binding::Section(section) => Some(section.shortcut()),
        }
    }

    pub fn menu_id(self) -> MenuId {
        match self {
            Binding::Command(command) => MenuId::Command(command),
            Binding::Section(section) => MenuId::Section(section),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap(BTreeMap<String, String>);

impl Keymap {
    pub fn new(overrides: &BTreeMap<String, String>) -> Self {
        Self(overrides.clone())
    }

    pub fn key(&self, binding: Binding) -> Option<String> {
        match self.0.get(&binding.name()) {
            Some(key) => (!key.is_empty()).then(|| key.clone()),
            None => binding.default_key().map(str::to_owned),
        }
    }

    pub fn set(&mut self, binding: Binding, key: Option<String>) {
        let key = key.unwrap_or_default();
        if binding.default_key().unwrap_or_default() == key {
            self.0.remove(&binding.name());
        } else {
            self.0.insert(binding.name(), key);
        }
    }

    pub fn owner(&self, key: &str, except: Binding) -> Option<Binding> {
        Binding::all().into_iter().find(|binding| {
            *binding != except && self.key(*binding).is_some_and(|owned| owned.eq_ignore_ascii_case(key))
        })
    }

    pub fn overrides(&self) -> BTreeMap<String, String> {
        self.0.clone()
    }
}

fn key_name(part: &str) -> Option<String> {
    let lower = part.to_ascii_lowercase();
    if let Some(number) = lower.strip_prefix('f').and_then(|number| number.parse::<u8>().ok())
        && (1..=24).contains(&number)
    {
        return Some(format!("F{number}"));
    }
    if let Some((_, name)) = NAMED.iter().find(|(alias, _)| *alias == lower) {
        return Some((*name).to_owned());
    }
    let mut chars = part.chars();
    match (chars.next(), chars.next()) {
        (Some(char), None) if char.is_ascii_graphic() && char != '+' => Some(char.to_ascii_uppercase().to_string()),
        _ => None,
    }
}

pub fn accelerator(text: &str) -> Option<String> {
    let mut modifiers = [false; 3];
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        match MODIFIERS.iter().position(|name| {
            name.eq_ignore_ascii_case(part) || (*name == "Ctrl" && part.eq_ignore_ascii_case("control"))
        }) {
            Some(index) => modifiers[index] = true,
            None if key.replace(key_name(part)?).is_some() => return None,
            None => {}
        }
    }
    let key = key?;
    let function = key.len() > 1 && key.starts_with('F') && key[1..].bytes().all(|byte| byte.is_ascii_digit());
    if !function && !modifiers.contains(&true) {
        return None;
    }
    let parts: Vec<&str> =
        MODIFIERS.iter().zip(modifiers).filter(|(_, on)| *on).map(|(name, _)| *name).chain([key.as_str()]).collect();
    Some(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keys_are_valid_and_unique() {
        let keys: Vec<&str> = Binding::all().into_iter().filter_map(Binding::default_key).collect();
        for key in &keys {
            assert_eq!(accelerator(key).as_deref(), Some(*key));
        }
        let mut unique = keys.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
        let mut names: Vec<String> = Binding::all().into_iter().map(Binding::name).collect();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn normalizes_accelerators() {
        assert_eq!(accelerator("shift+ctrl+k").as_deref(), Some("Ctrl+Shift+K"));
        assert_eq!(accelerator(" Control + pgdn ").as_deref(), Some("Ctrl+PageDown"));
        assert_eq!(accelerator("f7").as_deref(), Some("F7"));
        assert_eq!(accelerator("Alt+,").as_deref(), Some("Alt+,"));
        for invalid in ["K", "Ctrl", "Ctrl+K+L", "Ctrl+Nope", "", "Ctrl+", "F25", "Delete"] {
            assert_eq!(accelerator(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn overrides_replace_clear_and_reset_keys() {
        let mut keymap = Keymap::default();
        let play = Binding::Command(Command::PlayPause);
        keymap.set(play, Some("Ctrl+Shift+Space".into()));
        assert_eq!(keymap.key(play).as_deref(), Some("Ctrl+Shift+Space"));
        assert_eq!(keymap.owner("ctrl+shift+space", Binding::Command(Command::Stop)), Some(play));
        keymap.set(play, None);
        assert_eq!(keymap.key(play), None);
        keymap.set(play, Some("Ctrl+P".into()));
        assert!(keymap.overrides().is_empty());
        assert_eq!(keymap.owner("Ctrl+1", play), Some(Binding::Section(Section::Search)));
    }
}
