use super::key_editor::KeyEditor;
use super::*;
use std::collections::BTreeMap;

type Bindings = Rc<RefCell<BTreeMap<GlobalAction, String>>>;

pub(super) struct Hotkeys {
    enabled: CheckBox,
    bindings: Bindings,
}

fn hotkey_label(action: GlobalAction) -> String {
    match action {
        GlobalAction::PlayPause => fl!("menu-play-pause"),
        GlobalAction::Next => fl!("menu-next"),
        GlobalAction::Previous => fl!("menu-previous"),
        GlobalAction::SeekForward => fl!("menu-forward"),
        GlobalAction::SeekBack => fl!("menu-back"),
        GlobalAction::VolumeUp => fl!("menu-volume-up"),
        GlobalAction::VolumeDown => fl!("menu-volume-down"),
        GlobalAction::Like => fl!("menu-like"),
        GlobalAction::Dislike => fl!("menu-dislike"),
        GlobalAction::NowPlaying => fl!("menu-speak-now-playing"),
        GlobalAction::ShowWindow => fl!("hotkey-show-window"),
        GlobalAction::Voice => fl!("hotkey-voice"),
    }
    .replace('&', "")
}

fn rows(bindings: &BTreeMap<GlobalAction, String>) -> Vec<(String, Option<String>)> {
    GlobalAction::ALL.into_iter().map(|action| (hotkey_label(action), action.key_in(bindings))).collect()
}

impl Hotkeys {
    pub(super) fn new(notebook: &Notebook, old: &Settings, frame: Frame, dialog: Dialog) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-hotkeys"));
        let enabled = check(&panel, &fl!("settings-hotkeys-enabled"), old.global_hotkeys);
        add(&sizer, &enabled);
        let editor = KeyEditor::new(&panel, &sizer, &fl!("settings-hotkeys"));
        add(&sizer, &label(&panel, &fl!("settings-hotkeys-hint")));
        panel.set_sizer(sizer, true);
        let bindings: Bindings = Rc::new(RefCell::new(old.hotkeys.clone()));
        editor.fill(&rows(&bindings.borrow()));
        let update = {
            let bindings = bindings.clone();
            move |change: &dyn Fn(&mut BTreeMap<GlobalAction, String>)| {
                change(&mut bindings.borrow_mut());
                editor.fill(&rows(&bindings.borrow()));
            }
        };
        let selected = move || editor.selected().and_then(|row| GlobalAction::ALL.get(row)).copied();
        {
            let (bindings, update) = (bindings.clone(), update.clone());
            editor.change.on_click(move |_| {
                let Some(action) = selected() else { return };
                let current = action.key_in(&bindings.borrow()).unwrap_or_default();
                let prompt = fl!("hotkey-prompt", action = hotkey_label(action));
                let Some(text) = dialogs::ask_text(&frame, &prompt, &fl!("hotkey-change"), &current) else { return };
                match Hotkey::parse(&text) {
                    Some(hotkey) => update(&|map| {
                        map.insert(action, hotkey.to_string());
                    }),
                    None => dialogs::warn(&dialog, &fl!("hotkey-invalid", key = text), &fl!("hotkey-change")),
                }
            });
        }
        {
            let update = update.clone();
            editor.clear.on_click(move |_| {
                if let Some(action) = selected() {
                    update(&|map| {
                        map.insert(action, String::new());
                    });
                }
            });
        }
        editor.reset.on_click(move |_| update(&|map| *map = GlobalAction::defaults()));
        Self { enabled, bindings }
    }
}

impl Tab for Hotkeys {
    fn apply(&self, settings: &mut Settings) {
        settings.global_hotkeys = self.enabled.is_checked();
        settings.hotkeys = self.bindings.borrow().clone();
    }
}
