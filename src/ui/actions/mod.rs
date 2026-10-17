use super::command::{Keymap, MenuId, PLAYLIST_LIMIT, SleepChoice, escape};
use super::navigation::{Spot, Visit};
use super::view_model::{self, Entry, ItemAction, ListContext, PageButton, Status};
use super::*;
use crate::i18n::fl;
use crate::platform::edit::Edit;
use tactus_api::{Item, PlaylistEdit, Relation, Target, Track, View};
use tactus_core::announce::format_time;
use tactus_core::player::SleepTimer;
use tactus_core::player::{SEEK_STEP_MS, SPEED_STEP, VOLUME_STEP, wave};

mod bindings;
mod commands;
mod devices;
mod items;
mod library;
mod pages;

fn key(event: &WindowEventData) -> Option<i32> {
    match event {
        WindowEventData::Keyboard(key) => key.get_key_code(),
        _ => None,
    }
}

impl Ui {
    fn current_track(&self) -> Option<Track> {
        self.player.borrow().track.clone()
    }

    pub(crate) fn keymap(&self) -> Keymap {
        Keymap::new(&self.player.borrow().settings.app_keys)
    }

    pub(crate) fn rebuild_menu(&self) {
        self.frame.set_menu_bar(command::menu_bar(&self.keymap()));
        if let Some(menu) = self.frame.get_menu_bar() {
            menu.check_item(Command::Shuffle.id(), self.player.borrow().settings.shuffle);
        }
    }

    pub(crate) fn announce_feedback(&self, kind: Announcement) {
        let text = {
            let player = self.player.borrow();
            match kind {
                Announcement::Playback => {
                    if player.playing {
                        fl!("state-playing")
                    } else {
                        fl!("state-paused")
                    }
                }
                Announcement::Volume => fl!("announce-volume", percent = u32::from(player.settings.volume)),
                Announcement::Seek => view_model::position(&player),
                Announcement::Modes => bar::modes_text(&player),
                _ => return,
            }
        };
        self.announce(kind, &text, true);
    }

    pub(crate) fn announce_speed(&self) {
        let text = bar::speed_text(&self.player.borrow());
        self.announce(Announcement::Modes, &text, true);
    }

    fn copy(&self, text: &str) {
        if Clipboard::get().set_text(text) {
            let message = fl!("notice-link-copied");
            self.set_status(&message);
            self.speech.say(&message, true);
        }
    }
}
