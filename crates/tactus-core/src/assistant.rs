use crate::announce::Announcement;
use crate::log;
use crate::player::{Cue, Player};
use crate::voice::{self, MusicWhileListening};
use names::play;
use std::time::{Duration, Instant};
use tactus_api::{Item, View};
use tactus_voice::{Config, Control, Heard, Intent, Listener, NameKind, Phrases};

pub use tactus_voice::Place;

const TAP: Duration = Duration::from_millis(300);
const CONFIRM: Duration = Duration::from_millis(2_500);
const RESULTS: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Off,
    Starting,
    Cancelled,
    Nothing,
    NotUnderstood(String),
    NothingPlaying,
    Searching(String),
    NotFound(String),
    WillPlay(String),
    NoPlaylist(String),
    NoLikes,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Cue(Cue),
    Reply(Reply),
    Feedback(Announcement),
    Speed,
    OfferDownload,
    Open(Place),
    Search(String),
    Several(String),
    ReadSelection,
    WhatsPlaying,
    Download,
    GoToAlbum,
    GoToArtist,
    GoBack,
    Help,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Awaiting {
    Search { view: View, kind: NameKind, name: String },
    Likes,
}

#[derive(Default)]
pub struct Assistant {
    interface: String,
    listener: Option<Listener>,
    config: Option<Config>,
    phrases: Option<Phrases>,
    pressed: Option<Instant>,
    resume: bool,
    ducked: bool,
    pending: Option<(Item, Instant)>,
    awaiting: Option<Awaiting>,
}

impl Assistant {
    fn language(&self, player: &Player) -> &'static str {
        player.settings.voice.language_for(&self.interface)
    }

    fn ready(&self, player: &Player) -> bool {
        let code = self.language(player);
        voice::ready(code, player.settings.voice.size_for(code))
    }

    pub fn configure(&mut self, player: &mut Player, interface: &str) {
        self.interface = interface.to_owned();
        self.start(player);
    }

    fn start(&mut self, player: &mut Player) {
        let code = self.language(player);
        self.phrases = Some(voice::phrases(code));
        if !player.settings.voice.enabled || !self.ready(player) {
            self.listener = None;
            self.config = None;
            return;
        }
        let config = player.settings.voice.listener(code);
        if self.config.as_ref() != Some(&config) || self.listener.is_none() {
            self.listener = Some(Listener::start(config.clone()));
            self.config = Some(config);
        }
        player.load(&View::Collection, false);
    }

    pub fn examples(&mut self, player: &Player) -> Vec<String> {
        let code = self.language(player);
        self.phrases.get_or_insert_with(|| voice::phrases(code)).examples()
    }

    pub fn press(&mut self, player: &mut Player, now: Instant) -> Option<Outcome> {
        if self.pending.take().is_some() {
            return Some(Outcome::Reply(Reply::Cancelled));
        }
        if !player.settings.voice.enabled {
            return Some(Outcome::Reply(Reply::Off));
        }
        if !self.ready(player) {
            return Some(Outcome::OfferDownload);
        }
        if let Some(listener) = &self.listener {
            self.pressed = Some(now);
            listener.send(Control::Hold);
            return None;
        }
        self.start(player);
        Some(Outcome::Reply(Reply::Starting))
    }

    pub fn release(&mut self, now: Instant) {
        if let Some(pressed) = self.pressed.take()
            && let Some(listener) = &self.listener
        {
            listener.send(if now.duration_since(pressed) < TAP { Control::Tap } else { Control::Release });
        }
    }

    pub fn listen_once(&mut self, player: &mut Player, now: Instant) -> Option<Outcome> {
        match &self.listener {
            Some(listener) => {
                listener.send(Control::Tap);
                None
            }
            None => self.press(player, now),
        }
    }

    pub fn poll(&mut self, player: &mut Player, now: Instant) -> Vec<Outcome> {
        let heard = self.listener.as_ref().map(Listener::take).unwrap_or_default();
        let outcomes = heard.into_iter().flat_map(|heard| self.heard(player, heard, now)).collect();
        if let Some((item, _)) = self.pending.take_if(|(_, at)| now >= *at) {
            play(player, item);
        }
        outcomes
    }

    pub fn heard(&mut self, player: &mut Player, heard: Heard, now: Instant) -> Vec<Outcome> {
        if heard != Heard::Ready {
            self.music(player, matches!(heard, Heard::Listening | Heard::Woke));
        }
        match heard {
            Heard::Ready | Heard::Cancelled => Vec::new(),
            Heard::Listening | Heard::Woke => vec![Outcome::Cue(Cue::Listening)],
            Heard::Text(text) => self.text(player, &text, now),
            Heard::Nothing => vec![Outcome::Cue(Cue::NotUnderstood), Outcome::Reply(Reply::Nothing)],
            Heard::Failed(error) => {
                log::error(format_args!("voice: {error}"));
                self.listener = None;
                self.config = None;
                vec![Outcome::Reply(Reply::Failed(error))]
            }
        }
    }

    pub fn text(&mut self, player: &mut Player, text: &str, now: Instant) -> Vec<Outcome> {
        if player.settings.voice.log {
            log::info(format_args!("voice heard: {text}"));
        }
        let code = self.language(player);
        match self.phrases.get_or_insert_with(|| voice::phrases(code)).interpret(text) {
            Some(intent) => {
                std::iter::once(Outcome::Cue(Cue::Understood)).chain(self.run(player, intent, now)).collect()
            }
            None => vec![Outcome::Cue(Cue::NotUnderstood), Outcome::Reply(Reply::NotUnderstood(text.to_owned()))],
        }
    }

    fn music(&mut self, player: &mut Player, listening: bool) {
        match (player.settings.voice.music, listening) {
            (MusicWhileListening::Duck, true) if !self.ducked => {
                self.ducked = true;
                player.set_ducked(true);
            }
            (MusicWhileListening::Pause, true) if player.playing => {
                self.resume = true;
                player.set_playing(false);
            }
            (_, false) => {
                if std::mem::take(&mut self.ducked) {
                    player.set_ducked(false);
                }
                if std::mem::take(&mut self.resume) {
                    player.set_playing(true);
                }
            }
            _ => {}
        }
    }
}

mod intents;
mod names;
#[cfg(test)]
mod tests;
