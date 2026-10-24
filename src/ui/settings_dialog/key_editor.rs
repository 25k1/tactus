use super::*;
use crate::ui::command::accelerator;
use crate::ui::key_capture::{self, Captured, KeyPress};

#[derive(Clone, Copy)]
pub(super) enum KeyKind {
    Shortcut,
    Hotkey,
}

impl KeyKind {
    fn normalize(self, text: &str) -> Option<String> {
        match self {
            KeyKind::Shortcut => accelerator(text),
            KeyKind::Hotkey => Hotkey::parse(text).map(|hotkey| hotkey.to_string()),
        }
    }

    fn describe(self, press: KeyPress) -> Option<String> {
        match self {
            KeyKind::Shortcut => press.accelerator(),
            KeyKind::Hotkey => press.hotkey(),
        }
    }

    fn invalid(self, key: &str) -> String {
        match self {
            KeyKind::Shortcut => fl!("shortcut-invalid", key = key),
            KeyKind::Hotkey => fl!("hotkey-invalid", key = key),
        }
    }
}

pub(super) fn ask_key(ui: &Rc<Ui>, dialog: &Dialog, kind: KeyKind, action: &str, current: &str) -> Option<String> {
    let title = fl!("hotkey-change");
    ui.hotkeys.borrow_mut().take();
    let speaker = ui.clone();
    let captured = key_capture::capture(
        dialog,
        &title,
        &fl!("key-capture-prompt", action = action),
        current,
        move |press| kind.describe(press),
        move |text| speaker.speech.say(text, true),
    );
    ui.set_hotkeys();
    match captured {
        Captured::Key(key) => Some(key),
        Captured::Cancel => None,
        Captured::Type => {
            let text = dialogs::ask_text(&ui.frame, &fl!("hotkey-prompt", action = action), &title, current)?;
            let key = kind.normalize(&text);
            if key.is_none() {
                dialogs::warn(dialog, &kind.invalid(&text), &title);
            }
            key
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct KeyEditor {
    list: ListCtrl,
    pub(super) change: Button,
    pub(super) clear: Button,
    pub(super) reset: Button,
}

impl KeyEditor {
    pub(super) fn new(panel: &Panel, sizer: &BoxSizer, name: &str) -> Self {
        add(sizer, &label(panel, name));
        let list = ListCtrl::builder(panel).with_style(ListCtrlStyle::Report | ListCtrlStyle::SingleSel).build();
        list.set_accessibility_label(name);
        list.insert_column(0, &fl!("column-action"), ListColumnFormat::Left, 300);
        list.insert_column(1, &fl!("column-shortcut"), ListColumnFormat::Left, 220);
        sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 6);
        let line = row();
        let [change, clear, reset] =
            [fl!("hotkey-change"), fl!("hotkey-clear"), fl!("hotkey-reset")].map(|text| button(panel, &text));
        for control in [&change, &clear, &reset] {
            line.add(control, 0, centered(), 6);
        }
        sizer.add_sizer(&line, 0, SizerFlag::All, 6);
        Self { list, change, clear, reset }
    }

    pub(super) fn fill(&self, rows: &[(String, Option<String>)]) {
        let selected = self.list.get_first_selected_item().max(0);
        self.list.delete_all_items();
        for (row, (action, key)) in rows.iter().enumerate() {
            self.list.insert_item(row as i64, action, None);
            let key = key.clone().filter(|key| !key.is_empty()).unwrap_or_else(|| fl!("hotkey-none"));
            self.list.set_item_text_by_column(row as i64, 1, &key);
        }
        let state = ListItemState::Selected | ListItemState::Focused;
        self.list.set_item_state(i64::from(selected), state, state);
    }

    pub(super) fn selected(&self) -> Option<usize> {
        usize::try_from(self.list.get_first_selected_item()).ok()
    }
}
