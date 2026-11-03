mod account;
mod content;
mod control;
pub mod event;
mod library;
mod playback;
mod profile;
pub mod queue;
mod radio;
pub mod session;
pub mod sleep;
mod system;
mod undo;
mod voice;
pub mod wave;
pub mod ynison;

use crate::accounts::{AccountEntry, AccountList};
use crate::audio::{self, Loaded, Loader, Output, RodioOutput, Start};
use crate::download::{self, Download, DownloadState, Downloader, Job};
use crate::log;
use crate::media::{Media, MediaCommand, MediaEvent};
use crate::offline::Offline;
use crate::settings::Settings;
use crate::update;
pub use event::{Cue, Event, Notice};
pub use profile::Profile;
use queue::{Queue, Repeat};
use session::Session;
use sleep::Sleep;
pub use sleep::SleepTimer;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tactus_api::error::{Error, ErrorKind, Failure, Result};
use tactus_api::net;
use tactus_api::sync::Guard;
use tactus_api::{
    Account, AccountOption, AccountSettings, Capabilities, Item, Library, Lyrics, MusicService, Page, Play, Playlist,
    PlaylistEdit, PlaylistKey, Provider, ProviderKind, Relation, Section, SectionKind, Service, Target, Track, View,
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
#[cfg(not(test))]
const STREAM_RETRY_DELAYS: [Duration; 4] =
    [Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10), Duration::from_secs(30)];
#[cfg(test)]
const STREAM_RETRY_DELAYS: [Duration; 4] = [Duration::from_millis(1); 4];
pub const SEEK_STEP_MS: i64 = 10_000;
pub const VOLUME_STEP: i16 = 5;
pub const SPEED_STEP: f32 = 0.1;

type Update = Box<dyn FnOnce(&mut Player) + Send>;
pub type SharedProvider = std::sync::Arc<dyn Provider>;

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
    pub profile: Profile,
    pub queue: Queue,
    pub track: Option<Track>,
    pub playing: bool,
    pub position_ms: u64,
    pub downloads: Vec<Download>,
    pub offline: Offline,
    pub lyrics: Option<Lyrics>,
    pub lyric_line: Option<usize>,
    pub busy: usize,
    providers: Vec<SharedProvider>,
    others: Vec<Profile>,
    owner: String,
    events: Vec<Event>,
    sender: Sender<Update>,
    updates: Receiver<Update>,
    audio: Box<dyn Output>,
    loader: Loader,
    sleep: Option<Sleep>,
    wave: Wave,
    media: Option<Media>,
    downloader: Downloader,
    persist: bool,
    restart: Option<bool>,
    settings_dirty: bool,
    remote_queue: Option<String>,
    generation: u64,
    login_generation: u64,
    prefetched: Option<(String, Loaded)>,
    stream_retries: Option<usize>,
    prefetching: Option<String>,
    loading_more: HashSet<(View, usize)>,
    undo: Vec<Undo>,
    started: Instant,
    last_save: Instant,
    last_episode_check: Option<Instant>,
    ynison: ynison::Ynison,
    voice_install: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    wave_options: Option<tactus_api::WaveSettings>,
    generated_stream: std::sync::Arc<std::sync::Mutex<String>>,
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |elapsed| elapsed.as_nanos() as u64)
}

pub fn providers() -> Vec<SharedProvider> {
    vec![std::sync::Arc::new(tactus_yandex::YandexProvider)]
}

impl Player {
    pub fn new(settings: Settings, providers: Vec<SharedProvider>) -> Self {
        let output =
            RodioOutput::new(&settings.output_device, settings.volume, settings.speed, &settings.active_equalizer());
        Self::with_output(settings, providers, Box::new(output), audio::loader())
    }

    pub fn with_output(
        settings: Settings,
        providers: Vec<SharedProvider>,
        audio: Box<dyn Output>,
        loader: Loader,
    ) -> Self {
        let (sender, updates) = mpsc::channel();
        let guest = Profile::new(AccountEntry::default(), providers[0].clone());
        Self {
            audio,
            loader,
            sleep: None,
            settings,
            login: LoginState::Idle,
            profile: guest,
            queue: Queue::default(),
            track: None,
            playing: false,
            position_ms: 0,
            downloads: Vec::new(),
            offline: Offline::default(),
            lyrics: None,
            lyric_line: None,
            busy: 0,
            providers,
            others: Vec::new(),
            owner: String::new(),
            events: Vec::new(),
            sender,
            updates,
            wave: Wave::default(),
            media: None,
            downloader: Downloader::default(),
            persist: true,
            restart: None,
            settings_dirty: false,
            remote_queue: None,
            generation: 0,
            login_generation: 0,
            prefetched: None,
            stream_retries: None,
            prefetching: None,
            loading_more: HashSet::new(),
            undo: Vec::new(),
            started: Instant::now(),
            last_save: Instant::now(),
            last_episode_check: None,
            ynison: ynison::Ynison::default(),
            voice_install: None,
            wave_options: None,
            generated_stream: std::sync::Arc::default(),
        }
    }

    pub fn resume_after_restart(&mut self, playing: bool) {
        self.restart = Some(playing);
    }

    pub fn start(&mut self) {
        self.offline = Offline::load();
        let list = AccountList::load();
        self.owner = list.queue_owner;
        for entry in list.accounts {
            let Some(provider) = self.provider_for(entry.provider) else {
                continue;
            };
            let mut profile = Profile::new(entry, provider);
            profile.load_session();
            let previous = std::mem::replace(&mut self.profile, profile);
            if previous.entry.is_saved() {
                self.others.push(previous);
            }
        }
        if let Some(index) = self.others.iter().position(|profile| profile.id() == list.active) {
            std::mem::swap(&mut self.profile, &mut self.others[index]);
        }
        if !self.has_account(&self.owner) {
            self.owner = self.profile.id().to_owned();
        }
        for id in self.account_ids() {
            self.within(&id, Player::connect_saved);
        }
    }

    pub fn is_signed_in(&self) -> bool {
        self.profile.is_signed_in()
    }

    fn provider_for(&self, kind: ProviderKind) -> Option<SharedProvider> {
        self.providers.iter().find(|provider| provider.kind() == kind).cloned()
    }

    fn account_ids(&self) -> Vec<String> {
        std::iter::once(&self.profile).chain(&self.others).map(|profile| profile.id().to_owned()).collect()
    }

    fn has_account(&self, id: &str) -> bool {
        std::iter::once(&self.profile).chain(&self.others).any(|profile| profile.id() == id)
    }

    pub fn is_owner(&self) -> bool {
        self.profile.id() == self.owner
    }

    pub(super) fn within<R>(&mut self, id: &str, work: impl FnOnce(&mut Player) -> R) -> Option<R> {
        if self.profile.id() == id {
            return Some(work(self));
        }
        let index = self.others.iter().position(|profile| profile.id() == id)?;
        let home = self.profile.id().to_owned();
        std::mem::swap(&mut self.profile, &mut self.others[index]);
        let result = work(self);
        if let Some(index) = self.others.iter().position(|profile| profile.id() == home) {
            std::mem::swap(&mut self.profile, &mut self.others[index]);
        }
        Some(result)
    }

    pub fn as_owner<R: Default>(&mut self, work: impl FnOnce(&mut Player) -> R) -> R {
        let owner = self.owner.clone();
        self.within(&owner, work).unwrap_or_default()
    }

    pub(super) fn owner_profile(&self) -> &Profile {
        std::iter::once(&self.profile)
            .chain(&self.others)
            .find(|profile| profile.id() == self.owner)
            .unwrap_or(&self.profile)
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
        W: FnOnce(&dyn MusicService) -> Result<T> + Send + 'static,
        D: FnOnce(&mut Player, Result<T>) + Send + 'static,
    {
        let Some(service) = self.profile.service.clone() else {
            self.fail(Error::SignedOut);
            return;
        };
        let context = self.profile.id().to_owned();
        self.background(
            move || work(service.as_ref()),
            move |player, result| {
                player.within(&context, |player| done(player, result));
            },
        );
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
        W: FnOnce(&dyn MusicService) -> Result<T> + Send + 'static,
        D: FnOnce(&mut Player, T) + Send + 'static,
    {
        self.spawn(work, move |player, result| match result {
            Ok(value) => done(player, value),
            Err(error) => player.fail(error),
        });
    }

    fn quiet(&mut self, work: impl FnOnce(&dyn MusicService) -> Result<()> + Send + 'static) {
        if self.profile.service.is_some() {
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
        let updates: Vec<download::Update> = self.downloader.updates().collect();
        for update in updates {
            self.apply_download(update);
        }
        self.as_owner(Player::poll_owner);
        self.poll_episodes();
        for id in self.account_ids() {
            self.within(&id, Player::poll_connection);
        }
        self.flush_settings();
    }

    fn poll_owner(&mut self) {
        let media_events: Vec<MediaEvent> =
            self.media.as_ref().map(|media| media.events().collect()).unwrap_or_default();
        for event in media_events {
            self.handle_media(event);
        }
        self.audio.tick();
        self.poll_sleep();
        if self.audio.is_loaded() {
            self.poll_playback();
        }
        self.poll_ynison();
        if self.last_save.elapsed() >= SAVE_INTERVAL {
            if self.playing {
                self.save_session();
            }
            self.last_save = Instant::now();
        }
    }

    pub(super) fn save_session(&mut self) {
        if !self.persist {
            return;
        }
        if let Some(track) = self.track.clone() {
            self.profile.session.remember_position(&track, self.position_ms);
        }
        self.profile.session.tracks = self.queue.tracks().to_vec();
        self.profile.session.index = self.queue.index();
        self.profile.session.position_ms = self.position_ms;
        self.profile.session.wave = self.wave.seeds.clone();
        if let Err(error) = self.profile.session.save(&self.profile.entry) {
            log::warn(format_args!("session: {error}"));
        }
        self.last_save = Instant::now();
    }

    pub fn shutdown(&mut self) {
        self.as_owner(Player::save_session);
        self.flush_settings();
        self.audio.stop();
    }
}

#[cfg(test)]
pub(crate) mod tests;
