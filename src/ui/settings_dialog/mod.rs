mod account;
mod downloads;
mod equalizer;
mod general;
mod hotkeys;
mod key_editor;
mod lists;
mod playback;
mod remote;
mod shortcuts;
mod speech;
mod voice;

use super::*;
use crate::i18n::fl;
use crate::platform::{autostart, protocol};
use tactus_api::{AccountOption, Quality};
use tactus_core::audio::Gains;

type Flag = fn(&mut Settings) -> &mut bool;

trait Tab {
    fn apply(&self, settings: &mut Settings);
}

struct Flags(Vec<(CheckBox, Flag)>);

impl Flags {
    fn new(panel: &Panel, sizer: &BoxSizer, settings: &Settings, items: &[(String, Flag)]) -> Self {
        let mut probe = settings.clone();
        Self(
            items
                .iter()
                .map(|(text, flag)| {
                    let control = check(panel, text, *flag(&mut probe));
                    add(sizer, &control);
                    (control, *flag)
                })
                .collect(),
        )
    }

    fn apply(&self, settings: &mut Settings) {
        for (control, flag) in &self.0 {
            *flag(settings) = control.is_checked();
        }
    }
}

fn page(notebook: &Notebook, title: &str) -> (Panel, BoxSizer) {
    let panel = Panel::builder(notebook).build();
    notebook.add_page(&panel, title, false, None);
    (panel, column())
}

fn add(sizer: &BoxSizer, widget: &impl WxWidget) {
    sizer.add(widget, 0, SizerFlag::Expand | SizerFlag::All, 6);
}

fn quality_label(quality: Quality) -> String {
    match quality {
        Quality::Lossless => fl!("quality-lossless"),
        Quality::High => fl!("quality-high"),
        Quality::Low => fl!("quality-low"),
    }
}

fn quality_choice(parent: &Panel, quality: Quality) -> Choice {
    let labels: Vec<String> = Quality::ALL.into_iter().map(quality_label).collect();
    choice(parent, &labels, Quality::ALL.iter().position(|candidate| *candidate == quality).unwrap_or_default())
}

fn chosen_quality(control: &Choice) -> Quality {
    Quality::ALL[selection(control).min(Quality::ALL.len() - 1)]
}

fn editable(parent: &Panel, presets: &[&str], value: &str) -> ComboBox {
    let combo = ComboBox::builder(parent).build();
    for preset in presets {
        combo.append(preset);
    }
    combo.set_value(value);
    combo
}

fn buttons(dialog: &Dialog) -> StdDialogButtonSizer {
    let buttons = StdDialogButtonSizerBuilder::new().build();
    let ok = Button::builder(dialog).with_id(ID_OK).with_label(&fl!("dialog-ok")).build();
    let cancel = Button::builder(dialog).with_id(ID_CANCEL).with_label(&fl!("dialog-cancel")).build();
    buttons.add_button(&ok);
    buttons.add_button(&cancel);
    buttons.realize();
    ok.set_default();
    buttons
}

impl Ui {
    pub(super) fn show_settings(self: &Rc<Self>) {
        let old = self.settings();
        let dialog = Dialog::builder(&self.frame, &fl!("settings-title"))
            .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
            .with_size(660, 640)
            .build();
        let notebook = Notebook::builder(&dialog).build();
        let general = general::General::new(&notebook, &old);
        let focus = general.language;
        let preview = {
            let ui = self.clone();
            Rc::new(move |gains: &Gains| ui.act(|player| player.preview_equalizer(gains)))
        };
        let tabs: Vec<Box<dyn Tab>> = vec![
            Box::new(general),
            Box::new(playback::Playback::new(&notebook, &old)),
            Box::new(equalizer::Equalizer::new(&notebook, &old, preview)),
            Box::new(speech::Speech::new(&notebook, &old, self.speech.backend_name())),
            Box::new(voice::Voice::new(&notebook, &old, self)),
            Box::new(lists::Lists::new(&notebook, &old)),
            Box::new(shortcuts::Shortcuts::new(&notebook, &old, self, dialog)),
            Box::new(hotkeys::Hotkeys::new(&notebook, &old, self, dialog)),
            Box::new(downloads::Downloads::new(&notebook, &old)),
            Box::new(remote::Remote::new(&notebook, &old)),
        ];
        let account = account::Account::new(&notebook, &self.player.borrow().account_settings);
        notebook.set_selection(0);
        let sizer = column();
        sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 8);
        sizer.add_sizer(&buttons(&dialog), 0, SizerFlag::Expand | SizerFlag::All, 8);
        dialog.set_sizer(sizer, true);
        dialog.set_escape_id(ID_CANCEL);
        focus.set_focus();
        if dialog.show_modal() != ID_OK {
            self.act(|player| player.preview_equalizer(&old.active_equalizer()));
            dialog.destroy();
            return;
        }
        let mut new = old.clone();
        for tab in &tabs {
            tab.apply(&mut new);
        }
        let changes = account.changes();
        dialog.destroy();
        self.apply_settings(&old, &new, changes);
    }

    fn apply_settings(self: &Rc<Self>, old: &Settings, new: &Settings, account_changes: Vec<(AccountOption, bool)>) {
        let updated = new.clone();
        self.act(|player| player.update_settings(|settings| *settings = updated));
        for (option, enabled) in account_changes {
            self.act(|player| player.set_account_option(option, enabled));
        }
        if (&old.language, old.theme) != (&new.language, new.theme) {
            if dialogs::confirm(&self.frame, &fl!("settings-restart"), &fl!("settings-title")) {
                return self.restart(None);
            }
            self.set_status(&fl!("settings-restart-later"));
        }
        if (new.notifications && !old.notifications) || (new.episode_notifications && !old.episode_notifications) {
            crate::platform::notify::enable();
        }
        if old.tray != new.tray {
            self.set_tray(new.tray);
        }
        if (old.remote_enabled, old.remote_port, &old.remote_token)
            != (new.remote_enabled, new.remote_port, &new.remote_token)
        {
            self.set_remote(new.remote_enabled);
        }
        if (old.global_hotkeys, &old.hotkeys) != (new.global_hotkeys, &new.hotkeys) {
            self.set_hotkeys();
        }
        if new.autostart != autostart::is_enabled()
            && let Err(error) = autostart::set(new.autostart)
        {
            self.set_status(&error.to_string());
        }
        if new.open_links != protocol::is_enabled()
            && let Err(error) = protocol::set(new.open_links)
        {
            self.set_status(&error.to_string());
        }
        if old.columns != new.columns {
            self.browser.list.set_columns(&new.columns);
        }
        if old.app_keys != new.app_keys {
            self.rebuild_menu();
        }
        if old.voice != new.voice {
            self.start_voice();
        }
        if new.voice.enabled && !old.voice.enabled {
            self.offer_voice_download(new);
        }
        if old.track_format != new.track_format {
            self.each_player(|view, player| view.update_track(player));
        }
        self.publish_status();
    }

    pub(super) fn offer_voice_download(self: &Rc<Self>, settings: &Settings) {
        let code = settings.voice.language_for(&crate::i18n::current_language());
        let size = settings.voice.size_for(code);
        if tactus_core::voice::ready(code, size) || self.player.borrow().installing_voice() {
            return;
        }
        let megabytes = (tactus_core::voice::model(code, size).size + tactus_core::voice::RUNTIME.size) / 1_000_000;
        if dialogs::confirm(&self.frame, &fl!("voice-download-confirm", megabytes = megabytes), &fl!("settings-voice"))
        {
            self.act(|player| player.install_voice(code, size));
        }
    }
}
