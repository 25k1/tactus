use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tactus_api::error::{Error, Result};
use tactus_api::sync::Guard;
use tactus_api::{Codec, Play, Quality, Service, Track, net};
use tactus_stream::Buffer;
use tactus_winamp::{Connection, Store, log, tracks};

static CONNECTION: OnceLock<Connection> = OnceLock::new();
static TRACKS: LazyLock<Mutex<HashMap<String, Track>>> = LazyLock::new(Mutex::default);
static PENDING: LazyLock<Mutex<Pending>> = LazyLock::new(Mutex::default);

const BATCH: usize = 100;
const POLL: Duration = Duration::from_millis(50);

#[derive(Default)]
struct Pending {
    ids: HashSet<String>,
    running: bool,
}

pub fn init(store: Store) {
    let _ = CONNECTION.set(Connection::new(store));
}

fn connection() -> Result<&'static Connection> {
    CONNECTION.get().ok_or(Error::SignedOut)
}

pub fn service() -> Result<Service> {
    connection()?.service()
}

pub struct Source {
    pub buffer: Arc<Buffer>,
    pub codec: Codec,
}

pub fn source(id: &str, quality: Quality) -> Result<Source> {
    let source = service()?.audio_source(id, quality)?;
    Ok(Source { buffer: tactus_stream::fetch(&source)?, codec: source.codec })
}

pub fn quality() -> Quality {
    CONNECTION.get().map(|connection| connection.store().settings().quality).unwrap_or_default()
}

pub fn remember(tracks: impl IntoIterator<Item = Track>) {
    let mut known = TRACKS.guard();
    for track in tracks {
        known.insert(track.id.clone(), track);
    }
}

pub fn cached(id: &str) -> Option<Track> {
    TRACKS.guard().get(id).cloned()
}

pub fn track(id: &str) -> Result<Track> {
    if let Some(track) = cached(id) {
        return Ok(track);
    }
    let track = connection()?.track(id)?;
    remember([track.clone()]);
    Ok(track)
}

fn fetch(id: &str) {
    if TRACKS.guard().contains_key(id) {
        return;
    }
    let mut pending = PENDING.guard();
    pending.ids.insert(id.to_owned());
    if !std::mem::replace(&mut pending.running, true) {
        net::spawn_blocking(fetch_pending);
    }
}

fn fetch_pending() {
    loop {
        let batch: Vec<String> = {
            let mut pending = PENDING.guard();
            if pending.ids.is_empty() {
                pending.running = false;
                return;
            }
            pending.ids.iter().take(BATCH).cloned().collect()
        };
        match service().and_then(|service| service.tracks(&batch)) {
            Ok(tracks) => remember(tracks),
            Err(error) => log::write(format_args!("tracks: {error}")),
        }
        let mut pending = PENDING.guard();
        for id in &batch {
            pending.ids.remove(id);
        }
    }
}

pub fn track_within(id: &str, wait: Duration) -> Option<Track> {
    let deadline = Instant::now() + wait;
    fetch(id);
    loop {
        if let Some(track) = cached(id) {
            return Some(track);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(POLL);
    }
}

pub fn lookup(id: &str) -> Option<Track> {
    let track = cached(id);
    if track.is_none() {
        fetch(id);
    }
    track
}

pub fn report(track: Track, played_seconds: u32) {
    net::spawn_blocking(move || {
        let play = Play { track, played_seconds, from: tracks::PLAY_FROM.to_owned() };
        if let Err(error) = service().and_then(|service| service.report_play(&play)) {
            log::write(format_args!("play report: {error}"));
        }
    });
}
