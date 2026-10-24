use super::view_model;
use super::*;
use serde_json::json;
use tactus_api::View;
use tactus_core::player::{Cue, Event};

impl Ui {
    pub(super) fn apply_events(self: &Rc<Self>) {
        let events = match self.player.try_borrow_mut() {
            Ok(mut player) => player.take_events(),
            Err(_) => return,
        };
        if events.is_empty() {
            return;
        }
        let (mut status, mut queue, mut content) = (false, false, false);
        for event in events {
            match event {
                Event::Login => {
                    let was_signed_out = self.login.panel.is_shown();
                    self.show_screen();
                    match (was_signed_out, self.player.borrow().is_signed_in()) {
                        (true, true) => self.focus_content(),
                        (false, false) => self.login.get_code.set_focus(),
                        _ => {}
                    }
                }
                Event::Account => {
                    self.update_status();
                    status = true;
                }
                Event::Track => {
                    self.each_player(|view, player| view.update_track(player));
                    self.update_title();
                    self.announce_track();
                    self.notify_track();
                    self.update_tray_tooltip();
                    if *self.location.borrow() == Location::Section(Section::Lyrics) {
                        self.lyrics.update(&self.player.borrow());
                    }
                    status = true;
                }
                Event::Playback => {
                    self.each_player(|view, player| view.update_playback(player));
                    status = true;
                }
                Event::Position => {
                    self.bar.update_position(&self.player.borrow());
                    status = true;
                }
                Event::Volume => {
                    self.each_player(|view, player| view.update_volume(player));
                    status = true;
                }
                Event::Modes => {
                    self.each_player(|view, player| view.update_modes(player));
                    if let Some(menu) = self.frame.get_menu_bar() {
                        menu.check_item(Command::Shuffle.id(), self.player.borrow().settings.shuffle);
                    }
                    status = true;
                }
                Event::Queue => {
                    queue = true;
                    content |= *self.location.borrow() == Location::Section(Section::Queue);
                }
                Event::Library => {
                    status = true;
                    content |= self.content.borrow().subject.is_some();
                }
                Event::Playlists => {}
                Event::Page(view) => {
                    self.voice_page(&view);
                    if self.current_view().as_ref() == Some(&view) {
                        content = true;
                        if matches!(view, View::Search(_)) && self.focus_results.replace(false) {
                            self.render(None, None);
                            self.browser.list.list.set_focus();
                        }
                    }
                }
                Event::Open(view) => self.navigate(Location::View(view), true),
                Event::Downloads => {
                    content |= *self.location.borrow() == Location::Section(Section::Downloads);
                    self.publish_downloads();
                }
                Event::Lyrics => {
                    self.lyrics.update(&self.player.borrow());
                    self.publish_lyrics();
                }
                Event::LyricLine(line) => self.on_lyric_line(line),
                Event::Busy => self.update_status(),
                Event::Notice(notice) => {
                    if notice == tactus_core::player::Notice::VoiceReady {
                        self.start_voice();
                    }
                    let text = text::notice(&notice);
                    self.set_status(&text);
                    self.announce(notice.announcement(), &text, false);
                    if let Some(cue) = notice.cue() {
                        self.cue(cue);
                    }
                    self.remote_hub.broadcast("notice", json!({ "text": text }));
                    status = true;
                }
                Event::Error(failure) => {
                    let error = text::failure(&failure);
                    self.set_status(&error);
                    self.announce(Announcement::Errors, &error, false);
                    self.cue(Cue::Error);
                    self.remote_hub.broadcast("error", json!({ "text": error, "kind": format!("{:?}", failure.kind) }));
                }
                Event::Suggestions(text, suggestions) if self.browser.search.get_value().trim() == text => {
                    let mut texts: Vec<String> = suggestions.best.iter().filter_map(view_model::item_title).collect();
                    for text in suggestions.texts {
                        if !texts.contains(&text) {
                            texts.push(text);
                        }
                    }
                    self.browser.set_suggestions(&texts);
                }
                Event::Suggestions(..) => {}
                Event::TrackInfo(info) => {
                    dialogs::show_text(
                        &self.frame,
                        &crate::i18n::fl!("menu-track-info"),
                        &view_model::track_info(&info),
                    );
                }
                Event::AccountSettings => {}
                Event::NewEpisodes(episodes) => self.notify_episodes(&episodes),
                Event::Devices => {}
                Event::VoiceProgress(percent) => {
                    let text = crate::i18n::fl!("voice-downloading", percent = percent);
                    self.set_status(&text);
                    if percent % 10 == 0 {
                        self.announce(Announcement::Downloads, &text, false);
                    }
                }
                Event::Searches => {
                    self.show_search_history();
                    content |= *self.location.borrow() == Location::Section(Section::Search);
                }
                Event::UpdateAvailable(release) => self.offer_update(*release),
                Event::Updated(path) => self.restart(Some(&path)),
                Event::Raise => self.raise_window(),
                Event::Quit => self.execute(Command::Quit),
            }
        }
        if content {
            self.render(None, None);
        }
        if status {
            self.publish_status();
        }
        if queue {
            self.publish_queue();
        }
    }

    fn notify_episodes(&self, episodes: &[tactus_api::Track]) {
        let title = crate::i18n::fl!("toast-new-episodes", count = episodes.len());
        let body: Vec<String> = episodes.iter().take(3).map(tactus_api::Track::full_title).collect();
        self.announce(Announcement::Playlists, &title, false);
        if let Err(error) = crate::platform::notify::show(&title, &body.join("\n")) {
            tactus_core::log::warn(format_args!("notification: {error}"));
        }
    }

    pub(super) fn show_search_history(&self) {
        if self.browser.search.get_value().trim().is_empty() {
            let searches = self.player.borrow().searches().to_vec();
            self.browser.set_suggestions(&searches);
        }
    }

    fn update_status(&self) {
        let text = {
            let player = self.player.borrow();
            match (&player.account, player.busy > 0) {
                _ if player.disconnected => crate::i18n::fl!("status-offline"),
                (_, true) => crate::i18n::fl!("status-loading"),
                (Some(account), false) => crate::i18n::fl!("status-signed-in", name = account.name.clone()),
                (None, false) => String::new(),
            }
        };
        self.set_status(&text);
    }

    fn update_title(&self) {
        let title = match view_model::now_playing(&self.player.borrow()) {
            Some(text) => format!("{text} - Tactus"),
            None => "Tactus".to_owned(),
        };
        self.frame.set_title(&title);
    }

    fn offer_update(self: &Rc<Self>, release: tactus_core::update::Release) {
        let mut message = crate::i18n::fl!(
            "update-available",
            version = release.version.as_str(),
            current = tactus_core::update::CURRENT
        );
        if !release.notes.trim().is_empty() {
            message = format!("{message}\n\n{}", release.notes.trim());
        }
        self.announce(
            Announcement::Errors,
            &crate::i18n::fl!("update-found", version = release.version.as_str()),
            false,
        );
        if !self.frame.is_shown() {
            self.raise_window();
        }
        if dialogs::confirm(&self.frame, &message, &crate::i18n::fl!("menu-check-updates")) {
            self.set_status(&crate::i18n::fl!("update-downloading"));
            self.act(|player| player.install_update(release));
        }
    }

    pub(super) fn restart(self: &Rc<Self>, updated: Option<&std::path::Path>) {
        let playing = self.player.borrow().playing;
        let result = match updated {
            Some(path) => crate::platform::instance::relaunch(path, true, playing),
            None => std::env::current_exe().and_then(|path| crate::platform::instance::relaunch(&path, false, playing)),
        };
        if let Err(error) = result {
            tactus_core::log::error(format_args!("restart: {error}"));
            let message =
                if updated.is_some() { crate::i18n::fl!("update-restart") } else { crate::i18n::fl!("restart-failed") };
            return dialogs::warn(&self.frame, &message, "Tactus");
        }
        self.execute(Command::Quit);
    }

    fn notify_track(&self) {
        let hidden = !self.frame.is_shown() || self.frame.is_iconized();
        let toast = {
            let player = self.player.borrow();
            let track = player.track.as_ref().filter(|_| player.settings.notifications && player.playing && hidden);
            track.map(|track| {
                let album = Some(track.album.as_str()).filter(|album| *album != track.title);
                let details: Vec<&str> = [Some(track.artist.as_str()), album]
                    .into_iter()
                    .flatten()
                    .filter(|text| !text.is_empty())
                    .collect();
                (track.full_title(), details.join(" — "))
            })
        };
        if let Some((title, body)) = toast
            && let Err(error) = crate::platform::notify::show(&title, &body)
        {
            tactus_core::log::warn(format_args!("notification: {error}"));
        }
    }

    fn announce_track(&self) {
        let text = {
            let player = self.player.borrow();
            if !player.playing {
                return;
            }
            view_model::now_playing(&player)
        };
        if let Some(text) = text {
            self.announce(Announcement::Track, &text, true);
        }
    }

    fn on_lyric_line(&self, line: usize) {
        if *self.location.borrow() == Location::Section(Section::Lyrics) {
            self.lyrics.follow(line);
        }
        if let Some((text, time)) = self.lyric_text(line) {
            if self.player.borrow().playing {
                self.announce(Announcement::Lyrics, &text, false);
            }
            self.remote_hub.broadcast("lyric", json!({ "index": line, "text": text, "time": time }));
        }
    }
}
