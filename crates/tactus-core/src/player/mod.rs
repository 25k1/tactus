mod account;
mod content;
mod control;
pub mod event;
mod library;
mod playback;
pub mod queue;
pub mod session;
pub mod sleep;
mod system;
mod undo;
mod voice;
pub mod wave;
pub mod ynison;

use crate::audio::{self, Loaded, Loader, Output, RodioOutput, Start};
use crate::download::{self, Download, DownloadState, Downloader, Job};
use crate::log;
use crate::media::{Media, MediaCommand, MediaEvent};
use crate::offline::Offline;
use crate::settings::{self, Settings};
use crate::update;
pub use event::{Cue, Event, Notice};
use queue::{Queue, Repeat};
use session::Session;
use sleep::Sleep;
pub use sleep::SleepTimer;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tactus_api::error::{Error, ErrorKind, Failure, Result};
use tactus_api::net;
use tactus_api::{
    Account, AccountOption, AccountSettings, Item, Library, Lyrics, MusicService, Page, Play, Playlist, PlaylistEdit,
    PlaylistKey, Relation, Section, SectionKind, Service, Target, Track, View, auth,
};
use undo::{Change, Undo};
pub use wave::MY_WAVE;
use wave::Wave;

const PLAY_FROM: &str = "desktop_win-own_tracks-track-default";
const PREFETCH_MS: u64 = 30_000;
const SAVE_INTERVAL: Duration = Duration::from_secs(15);
const FINISH_GUARD: Duration = Duration::from_secs(2);
const EPISODE_CHECK_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(30);
pub const SEEK_STEP_MS: i64 = 10_000;
pub const VOLUME_STEP: i16 = 5;
pub const SPEED_STEP: f32 = 0.1;

type Update = Box<dyn FnOnce(&mut Player) + Send>;
pub type Connect = fn(&str) -> Result<Service>;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum LoginState {
    #[default]
    Idle,
    Requesting,
    Waiting {
        code: String,
        url: String,
    },
    Failed(Failure),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageState {
    Loading,
    Ready(Page),
    Failed(Failure),
}

pub struct Player {
    pub settings: Settings,
    pub login: LoginState,
    pub account: Option<Account>,
    pub account_settings: AccountSettings,
    pub library: Library,
    pub playlists: Vec<Playlist>,
    pub queue: Queue,
    pub track: Option<Track>,
    pub playing: bool,
    pub position_ms: u64,
    pub pages: HashMap<View, PageState>,
    pub downloads: Vec<Download>,
    pub offline: Offline,
    pub disconnected: bool,
    pub lyrics: Option<Lyrics>,
    pub lyric_line: Option<usize>,
    pub busy: usize,
    connect: Connect,
    service: Option<Service>,
    events: Vec<Event>,
    sender: Sender<Update>,
    updates: Receiver<Update>,
    audio: Box<dyn Output>,
    loader: Loader,
    sleep: Option<Sleep>,
    wave: Wave,
    media: Option<Media>,
    downloader: Downloader,
    session: Session,
    persist: bool,
    settings_dirty: bool,
    remote_queue: Option<String>,
    generation: u64,
    login_generation: u64,
    prefetched: Option<(String, Loaded)>,
    prefetching: Option<String>,
    loading_more: HashSet<(View, usize)>,
    undo: Vec<Undo>,
    started: Instant,
    last_save: Instant,
    last_episode_check: Option<Instant>,
    last_connect: Option<Instant>,
    ynison: ynison::Ynison,
    voice_install: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |elapsed| elapsed.as_nanos() as u64)
}

pub fn connect_yandex(token: &str) -> Result<Service> {
    Ok(std::sync::Arc::new(tactus_api::YandexService::new(token)?))
}

impl Player {
    pub fn new(settings: Settings, connect: Connect) -> Self {
        let output =
            RodioOutput::new(&settings.output_device, settings.volume, settings.speed, &settings.active_equalizer());
        Self::with_output(settings, connect, Box::new(output), audio::loader())
    }

    pub fn with_output(settings: Settings, connect: Connect, audio: Box<dyn Output>, loader: Loader) -> Self {
        let (sender, updates) = mpsc::channel();
        Self {
            audio,
            loader,
            sleep: None,
            settings,
            login: LoginState::Idle,
            account: None,
            account_settings: AccountSettings::new(),
            library: Library::default(),
            playlists: Vec::new(),
            queue: Queue::default(),
            track: None,
            playing: false,
            position_ms: 0,
            pages: HashMap::new(),
            downloads: Vec::new(),
            offline: Offline::default(),
            disconnected: false,
            lyrics: None,
            lyric_line: None,
            busy: 0,
            connect,
            service: None,
            events: Vec::new(),
            sender,
            updates,
            wave: Wave::default(),
            media: None,
            downloader: Downloader::default(),
            session: Session::default(),
            persist: true,
            settings_dirty: false,
            remote_queue: None,
            generation: 0,
            login_generation: 0,
            prefetched: None,
            prefetching: None,
            loading_more: HashSet::new(),
            undo: Vec::new(),
            started: Instant::now(),
            last_save: Instant::now(),
            last_episode_check: None,
            last_connect: None,
            ynison: ynison::Ynison::default(),
            voice_install: None,
        }
    }

    pub fn start(&mut self) {
        self.session = Session::load();
        self.offline = Offline::load();
        if let Some(token) = settings::load_token() {
            self.connect_with(&token);
        }
    }

    pub fn is_signed_in(&self) -> bool {
        self.service.is_some()
    }

    pub fn uid(&self) -> u64 {
        self.account.as_ref().map_or(0, |account| account.uid)
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn emit(&mut self, event: Event) {
        if event.repeatable() || !self.events.contains(&event) {
            self.events.push(event);
        }
    }

    fn notice(&mut self, notice: Notice) {
        self.emit(Event::Notice(notice));
    }

    fn fail(&mut self, error: Error) {
        log::error(&error);
        self.emit(Event::Error(error.failure()));
    }

    fn set_busy(&mut self, busy: usize) {
        if (self.busy == 0) != (busy == 0) {
            self.emit(Event::Busy);
        }
        self.busy = busy;
    }

    fn spawn<T, W, D>(&mut self, work: W, done: D)
    where
        T: Send + 'static,
        W: FnOnce(&dyn MusicService, u64) -> Result<T> + Send + 'static,
        D: FnOnce(&mut Player, Result<T>) + Send + 'static,
    {
        let Some(service) = self.service.clone() else {
            self.fail(Error::SignedOut);
            return;
        };
        let uid = self.uid();
        self.background(move || work(service.as_ref(), uid), done);
    }

    fn background<T, W, D>(&mut self, work: W, done: D)
    where
        T: Send + 'static,
        W: FnOnce() -> Result<T> + Send + 'static,
        D: FnOnce(&mut Player, Result<T>) + Send + 'static,
    {
        let sender = self.sender.clone();
        self.set_busy(self.busy + 1);
        net::spawn_blocking(move || {
            let result = work();
            let _ = sender.send(Box::new(move |player: &mut Player| {
                player.set_busy(player.busy.saturating_sub(1));
                done(player, result);
            }));
        });
    }

    fn request<T, W, D>(&mut self, work: W, done: D)
    where
        T: Send + 'static,
        W: FnOnce(&dyn MusicService, u64) -> Result<T> + Send + 'static,
        D: FnOnce(&mut Player, T) + Send + 'static,
    {
        self.spawn(work, move |player, result| match result {
            Ok(value) => done(player, value),
            Err(error) => player.fail(error),
        });
    }

    fn quiet(&mut self, work: impl FnOnce(&dyn MusicService, u64) -> Result<()> + Send + 'static) {
        if self.service.is_some() {
            self.spawn(work, |_, result| {
                if let Err(error) = result {
                    log::warn(&error);
                }
            });
        }
    }

    pub fn poll(&mut self) {
        while let Ok(update) = self.updates.try_recv() {
            update(self);
        }
        let media_events: Vec<MediaEvent> =
            self.media.as_ref().map(|media| media.events().collect()).unwrap_or_default();
        for event in media_events {
            self.handle_media(event);
        }
        let updates: Vec<download::Update> = self.downloader.updates().collect();
        for update in updates {
            self.apply_download(update);
        }
        self.audio.tick();
        self.poll_sleep();
        if self.audio.is_loaded() {
            self.poll_playback();
        }
        self.poll_episodes();
        self.poll_connection();
        self.poll_ynison();
        if self.last_save.elapsed() >= SAVE_INTERVAL {
            if self.playing {
                self.save_session();
            }
            self.last_save = Instant::now();
        }
        self.flush_settings();
    }

    pub(super) fn save_session(&mut self) {
        if !self.persist {
            return;
        }
        if let Some(track) = self.track.clone() {
            self.session.remember_position(&track, self.position_ms);
        }
        self.session.tracks = self.queue.tracks().to_vec();
        self.session.index = self.queue.index();
        self.session.position_ms = self.position_ms;
        self.session.wave = self.wave.seeds.clone();
        if let Err(error) = self.session.save() {
            log::warn(format_args!("session: {error}"));
        }
        self.last_save = Instant::now();
    }

    pub fn shutdown(&mut self) {
        self.save_session();
        self.flush_settings();
        self.audio.stop();
    }
}

#[cfg(test)]
pub(crate) mod tests;
