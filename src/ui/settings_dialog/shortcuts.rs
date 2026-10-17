use super::key_editor::KeyEditor;
use super::*;
use crate::ui::command::{Binding, Keymap, accelerator};

pub(super) struct Shortcuts {
    keymap: Rc<RefCell<Keymap>>,
}

fn rows(keymap: &Keymap) -> Vec<(String, Option<String>)> {
    Binding::all().into_iter().map(|binding| (binding.label(), keymap.key(binding))).collect()
}

impl Shortcuts {
    pub(super) fn new(notebook: &Notebook, old: &Settings, frame: Frame, dialog: Dialog) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-shortcuts"));
        let editor = KeyEditor::new(&panel, &sizer, &fl!("settings-shortcuts"));
        add(&sizer, &label(&panel, &fl!("settings-shortcuts-hint")));
        panel.set_sizer(sizer, true);
        let keymap = Rc::new(RefCell::new(Keymap::new(&old.app_keys)));
        editor.fill(&rows(&keymap.borrow()));
        let selected = move || editor.selected().and_then(|row| Binding::all().get(row).copied());
        let update = {
            let keymap = keymap.clone();
            move |change: &dyn Fn(&mut Keymap)| {
                change(&mut keymap.borrow_mut());
                editor.fill(&rows(&keymap.borrow()));
            }
        };
        {
            let (keymap, update) = (keymap.clone(), update.clone());
            editor.change.on_click(move |_| {
                let Some(binding) = selected() else { return };
                let current = keymap.borrow().key(binding).unwrap_or_default();
                let prompt = fl!("hotkey-prompt", action = binding.label());
                let Some(text) = dialogs::ask_text(&frame, &prompt, &fl!("hotkey-change"), &current) else { return };
                let Some(key) = accelerator(&text) else {
                    return dialogs::warn(&dialog, &fl!("shortcut-invalid", key = text), &fl!("hotkey-change"));
                };
                let owner = keymap.borrow().owner(&key, binding);
                if let Some(owner) = owner {
                    let question = fl!("shortcut-taken", key = key.as_str(), action = owner.label());
                    if !dialogs::confirm(&frame, &question, &fl!("hotkey-change")) {
                        return;
                    }
                }
                update(&|keymap| {
                    if let Some(owner) = owner {
                        keymap.set(owner, None);
                    }
                    keymap.set(binding, Some(key.clone()));
                });
            });
        }
        {
            let update = update.clone();
            editor.clear.on_click(move |_| {
                if let Some(binding) = selected() {
                    update(&|keymap| keymap.set(binding, None));
                }
            });
        }
        editor.reset.on_click(move |_| update(&|keymap| *keymap = Keymap::default()));
        Self { keymap }
    }
}

impl Tab for Shortcuts {
    fn apply(&self, settings: &mut Settings) {
        settings.app_keys = self.keymap.borrow().overrides();
    }
}
