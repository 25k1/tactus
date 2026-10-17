use super::view_model;
use super::*;
use crate::i18n::fl;
use tactus_api::View;
use tactus_core::assistant::{Assistant, Outcome, Place, Reply};

fn place_section(place: Place) -> Section {
    match place {
        Place::Search => Section::Search,
        Place::Wave => Section::Wave,
        Place::ForYou => Section::ForYou,
        Place::Collection => Section::Collection,
        Place::Playlists => Section::Playlists,
        Place::Radio => Section::Radio,
        Place::Podcasts => Section::Podcasts,
        Place::History => Section::History,
        Place::Queue => Section::Queue,
        Place::Lyrics => Section::Lyrics,
        Place::Downloads => Section::Downloads,
    }
}

fn reply_text(reply: Reply) -> String {
    match reply {
        Reply::Off => fl!("voice-off"),
        Reply::Starting => fl!("voice-starting"),
        Reply::Cancelled => fl!("voice-cancelled"),
        Reply::Nothing => fl!("voice-nothing"),
        Reply::NotUnderstood(text) => fl!("voice-not-understood", text = text),
        Reply::NothingPlaying => fl!("now-playing-nothing"),
        Reply::Searching(name) => fl!("voice-searching", name = name),
        Reply::NotFound(name) => fl!("voice-not-found", name = name),
        Reply::WillPlay(title) => fl!("voice-will-play", title = title),
        Reply::NoPlaylist(name) => fl!("voice-no-playlist", name = name),
        Reply::NoLikes => fl!("voice-no-likes"),
        Reply::Failed(error) => fl!("voice-failed", error = error),
    }
}

impl Ui {
    fn assist<T: IntoIterator<Item = Outcome>>(self: &Rc<Self>, run: impl FnOnce(&mut Assistant, &mut Player) -> T) {
        let outcomes: Vec<Outcome> = match self.player.try_borrow_mut() {
            Ok(mut player) => run(&mut self.voice.borrow_mut(), &mut player).into_iter().collect(),
            Err(_) => return,
        };
        self.apply_events();
        for outcome in outcomes {
            self.voice_outcome(outcome);
        }
    }

    pub(super) fn start_voice(self: &Rc<Self>) {
        self.assist(|assistant, player| {
            assistant.configure(player, &crate::i18n::current_language());
            None
        });
    }

    pub(super) fn voice_pressed(self: &Rc<Self>) {
        self.assist(|assistant, player| assistant.press(player, Instant::now()));
    }

    pub(super) fn voice_released(&self) {
        self.voice.borrow_mut().release(Instant::now());
    }

    pub(super) fn listen_once(self: &Rc<Self>) {
        self.assist(|assistant, player| assistant.listen_once(player, Instant::now()));
    }

    pub(super) fn poll_voice(self: &Rc<Self>) {
        self.assist(|assistant, player| assistant.poll(player, Instant::now()));
    }

    pub(super) fn voice_text(self: &Rc<Self>, text: &str) {
        self.assist(|assistant, player| assistant.text(player, text, Instant::now()));
    }

    pub(super) fn voice_page(self: &Rc<Self>, view: &View) {
        if self.voice.borrow().waits_for(view) {
            self.assist(|assistant, player| assistant.page_ready(player, view, Instant::now()));
        }
    }

    fn reply(&self, text: &str) {
        self.set_status(text);
        self.announce(Announcement::Voice, text, true);
    }

    fn voice_outcome(self: &Rc<Self>, outcome: Outcome) {
        match outcome {
            Outcome::Cue(cue) => self.cue(cue),
            Outcome::Reply(reply) => self.reply(&reply_text(reply)),
            Outcome::Feedback(kind) => self.announce_feedback(kind),
            Outcome::Speed => self.announce_speed(),
            Outcome::OfferDownload => self.offer_voice_download(&self.settings()),
            Outcome::Open(place) => self.navigate(Location::Section(place_section(place)), true),
            Outcome::Search(text) => {
                self.browser.search.set_value(&text);
                self.run_search();
            }
            Outcome::Several(name) => {
                self.browser.search.set_value(&name);
                self.navigate(Location::Section(Section::Search), true);
                self.reply(&fl!("voice-several", name = name));
            }
            Outcome::ReadSelection => {
                let text = self.browser.list.selected().and_then(|row| self.browser.list.entry(row)).map_or_else(
                    || fl!("voice-nothing-selected"),
                    |entry| view_model::join_present(view_model::columns(&entry), ", "),
                );
                self.reply(&text);
            }
            Outcome::WhatsPlaying => self.execute(Command::SpeakNowPlaying),
            Outcome::Download => self.execute(Command::Download),
            Outcome::GoToAlbum => self.execute(Command::GoToAlbum),
            Outcome::GoToArtist => self.execute(Command::GoToArtist),
            Outcome::GoBack => self.execute(Command::Back),
            Outcome::Help => self.execute(Command::VoiceHelp),
        }
    }

    pub(super) fn show_voice_help(&self) {
        let examples = self.voice.borrow_mut().examples(&self.player.borrow());
        let key = GlobalAction::Voice.key_in(&self.player.borrow().settings.hotkeys).unwrap_or_default();
        let text = format!("{}\n\n{}", fl!("voice-help-intro", key = key), examples.join("\n"));
        dialogs::show_text(&self.frame, &fl!("menu-voice-help"), &text);
    }
}
