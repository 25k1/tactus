use super::command::accelerator;
use super::dialogs::caption;
use super::widgets::*;
use crate::i18n::fl;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tactus_core::hotkey::Hotkey;
use wxdragon::prelude::*;

const NAMED: [(i32, u32, &str); 23] = [
    (WXK_BACK, 0x08, "Back"),
    (WXK_TAB, 0x09, "Tab"),
    (WXK_RETURN, 0x0D, "Enter"),
    (WXK_ESCAPE, 0x1B, "Escape"),
    (WXK_SPACE, 0x20, "Space"),
    (WXK_PAGEUP, 0x21, "PageUp"),
    (WXK_PAGEDOWN, 0x22, "PageDown"),
    (WXK_END, 0x23, "End"),
    (WXK_HOME, 0x24, "Home"),
    (WXK_LEFT, 0x25, "Left"),
    (WXK_UP, 0x26, "Up"),
    (WXK_RIGHT, 0x27, "Right"),
    (WXK_DOWN, 0x28, "Down"),
    (WXK_INSERT, 0x2D, "Insert"),
    (WXK_DELETE, 0x2E, "Delete"),
    (WXK_PAUSE, 0x13, ""),
    (WXK_VOLUME_MUTE, 0xAD, ""),
    (WXK_VOLUME_DOWN, 0xAE, ""),
    (WXK_VOLUME_UP, 0xAF, ""),
    (WXK_MEDIA_NEXT_TRACK, 0xB0, ""),
    (WXK_MEDIA_PREV_TRACK, 0xB1, ""),
    (WXK_MEDIA_STOP, 0xB2, ""),
    (WXK_MEDIA_PLAY_PAUSE, 0xB3, ""),
];

const PUNCTUATION: [(char, u32); 11] = [
    ('.', 0xBE),
    (',', 0xBC),
    ('-', 0xBD),
    ('=', 0xBB),
    ('/', 0xBF),
    (';', 0xBA),
    ('\'', 0xDE),
    ('[', 0xDB),
    (']', 0xDD),
    ('\\', 0xDC),
    ('`', 0xC0),
];

const MODIFIER_KEYS: [i32; 6] = [WXK_SHIFT, WXK_ALT, WXK_CONTROL, WXK_MENU, WXK_WINDOWS_LEFT, WXK_WINDOWS_RIGHT];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub held: [bool; 4],
    pub key: u32,
}

impl KeyPress {
    pub fn new(code: i32, held: [bool; 4]) -> Option<Self> {
        let key = match code {
            0x30..=0x39 | 0x41..=0x5A => code as u32,
            WXK_F1..=WXK_F24 => 0x70 + (code - WXK_F1) as u32,
            WXK_NUMPAD0..=WXK_NUMPAD9 => 0x60 + (code - WXK_NUMPAD0) as u32,
            _ => NAMED.iter().find(|(wx, ..)| *wx == code).map(|(_, key, _)| *key).or_else(|| {
                let char = char::from_u32(u32::try_from(code).ok()?)?;
                PUNCTUATION.iter().find(|(punctuation, _)| *punctuation == char).map(|(_, key)| *key)
            })?,
        };
        Some(Self { held, key })
    }

    pub fn accelerator(self) -> Option<String> {
        let [ctrl, alt, shift, win] = self.held;
        if win {
            return None;
        }
        let key = match self.key {
            0x30..=0x39 | 0x41..=0x5A => char::from_u32(self.key)?.to_string(),
            0x70..=0x87 => format!("F{}", self.key - 0x6F),
            key => NAMED
                .iter()
                .find(|(_, code, name)| *code == key && !name.is_empty())
                .map(|(.., name)| (*name).to_owned())
                .or_else(|| PUNCTUATION.iter().find(|(_, code)| *code == key).map(|(char, _)| char.to_string()))?,
        };
        let parts: Vec<&str> = [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift")]
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
            .chain([key.as_str()])
            .collect();
        accelerator(&parts.join("+"))
    }

    pub fn hotkey(self) -> Option<String> {
        Hotkey::pressed(self.held, self.key).map(|hotkey| hotkey.to_string())
    }
}

pub enum Captured {
    Key(String),
    Type,
    Cancel,
}

pub fn capture(
    parent: &impl WxWidget,
    title: &str,
    prompt: &str,
    current: &str,
    describe: impl Fn(KeyPress) -> Option<String> + 'static,
    speak: impl Fn(&str) + 'static,
) -> Captured {
    let dialog = Dialog::builder(parent, &caption(title)).build();
    let sizer = column();
    sizer.add(&label(&dialog, prompt), 0, SizerFlag::Expand | SizerFlag::All, 8);
    let shown = field(&sizer, &dialog, &fl!("key-capture-field"), true, || {
        TextCtrl::builder(&dialog).with_style(TextCtrlStyle::ReadOnly).build()
    });
    shown.change_value(current);
    let line = row();
    let ok = Button::builder(&dialog).with_id(ID_OK).with_label(&fl!("dialog-ok")).build();
    let typed_in = button(&dialog, &fl!("key-capture-type"));
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&fl!("dialog-cancel")).build();
    for control in [&ok, &typed_in, &cancel] {
        line.add(control, 0, centered(), 6);
    }
    sizer.add_sizer(&line, 0, SizerFlag::AlignRight | SizerFlag::All, 8);
    dialog.set_sizer_and_fit(sizer, true);
    ok.set_default();
    let chosen: Rc<RefCell<Option<String>>> = Rc::default();
    let typing = Rc::new(Cell::new(false));
    {
        let typing = typing.clone();
        typed_in.on_click(move |_| {
            typing.set(true);
            dialog.end_modal(ID_CANCEL);
        });
    }
    {
        let chosen = chosen.clone();
        WindowEvents::on_key_down(&shown, move |event| {
            let WindowEventData::Keyboard(key) = &event else { return event.skip(true) };
            let Some(code) = key.get_key_code() else { return event.skip(true) };
            let held = [key.control_down(), key.alt_down(), key.shift_down(), key.meta_down()];
            if MODIFIER_KEYS.contains(&code) {
                return;
            }
            match KeyPress::new(code, held).and_then(&describe) {
                Some(text) => {
                    shown.change_value(&text);
                    speak(&text);
                    *chosen.borrow_mut() = Some(text);
                }
                None => speak(&fl!("key-capture-unusable")),
            }
        });
    }
    dialog.set_escape_id(ID_CANCEL);
    shown.set_focus();
    let confirmed = dialog.show_modal() == ID_OK;
    dialog.destroy();
    match chosen.take() {
        Some(key) if confirmed => Captured::Key(key),
        _ if typing.get() => Captured::Type,
        _ => Captured::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_keys_become_shortcuts_and_hotkeys() {
        let press = |code: i32, held| KeyPress::new(code, held).unwrap();
        let ctrl = [true, false, false, false];
        assert_eq!(press(i32::from(b'.'), ctrl).accelerator().as_deref(), Some("Ctrl+."));
        assert_eq!(press(i32::from(b'K'), [true, false, true, false]).accelerator().as_deref(), Some("Ctrl+Shift+K"));
        assert_eq!(press(WXK_F5, [false; 4]).accelerator().as_deref(), Some("F5"));
        assert_eq!(press(WXK_PAGEDOWN, ctrl).accelerator().as_deref(), Some("Ctrl+PageDown"));
        assert_eq!(press(i32::from(b'K'), [false; 4]).accelerator(), None);
        assert_eq!(press(i32::from(b'K'), [false, false, false, true]).accelerator(), None);
        assert_eq!(
            press(i32::from(b'.'), [true, true, true, false]).hotkey().as_deref(),
            Some("Ctrl+Alt+Shift+Period")
        );
        assert_eq!(press(WXK_MEDIA_PLAY_PAUSE, [false; 4]).hotkey().as_deref(), Some("MediaPlayPause"));
        assert_eq!(press(WXK_NUMPAD5, [false, true, false, true]).hotkey().as_deref(), Some("Alt+Win+Num5"));
        assert_eq!(press(WXK_MEDIA_PLAY_PAUSE, [false; 4]).accelerator(), None);
        assert_eq!(KeyPress::new(WXK_SHIFT, [false; 4]), None);
    }
}
