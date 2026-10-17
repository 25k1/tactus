use crate::error::{Error, Result};
use crate::parse::Json;
use serde_json::{Map, Value, json};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const REDIRECTOR: &str = "wss://ynison.music.yandex.ru/redirector.YnisonRedirectService/GetRedirectToYnison";
const STATE_PATH: &str = "/ynison_state.YnisonStateService/PutYnisonState";
const FROM: &str = "desktop_win-own_tracks-track-default";
const READ_TIMEOUT: Duration = Duration::from_millis(250);
const RETRY: Duration = Duration::from_secs(30);

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub volume: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playing {
    pub paused: bool,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub speed: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Playable {
    pub id: String,
    pub album_id: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shared {
    pub devices: Vec<Device>,
    pub active: Option<String>,
    pub queue: Vec<Playable>,
    pub index: Option<usize>,
    pub playing: Playing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outgoing {
    Full { queue: Vec<Playable>, index: Option<usize>, playing: Playing, active: bool },
    Status(Playing),
    Queue { queue: Vec<Playable>, index: Option<usize>, playing: Playing },
    Activate(String),
    Volume { device: String, volume: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    State(Shared),
    Unavailable(String),
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn failure(error: impl std::fmt::Display) -> Error {
    Error::Api(format!("ynison: {error}"))
}

pub struct Versions {
    device: String,
    counter: AtomicU64,
}

impl Versions {
    pub fn new(device: &str) -> Self {
        Self { device: device.to_owned(), counter: AtomicU64::new(now_ms() * 1000) }
    }

    fn next(&self) -> Value {
        let version = self.counter.fetch_add(1, Ordering::Relaxed);
        json!({ "device_id": self.device, "version": version, "timestamp_ms": now_ms() })
    }
}

fn status(playing: &Playing, versions: &Versions) -> Value {
    json!({
        "duration_ms": playing.duration_ms,
        "paused": playing.paused,
        "playback_speed": playing.speed,
        "progress_ms": playing.progress_ms,
        "version": versions.next()
    })
}

fn queue(tracks: &[Playable], index: Option<usize>, versions: &Versions) -> Value {
    let playables: Vec<Value> = tracks
        .iter()
        .map(|track| {
            json!({ "playable_id": track.id, "album_id_optional": track.album_id, "playable_type": "TRACK", "from": FROM, "title": "" })
        })
        .collect();
    json!({
        "current_playable_index": index.map_or(-1, |index| index as i64),
        "entity_id": "",
        "entity_type": "VARIOUS",
        "playable_list": playables,
        "options": { "repeat_mode": "NONE" },
        "entity_context": "BASED_ON_ENTITY_BY_DEFAULT",
        "version": versions.next(),
        "from_optional": ""
    })
}

pub fn message(identity: &Identity, versions: &Versions, outgoing: &Outgoing) -> Value {
    let mut message = match outgoing {
        Outgoing::Full { queue: tracks, index, playing, active } => json!({
            "update_full_state": {
                "player_state": { "player_queue": queue(tracks, *index, versions), "status": status(playing, versions) },
                "device": {
                    "capabilities": { "can_be_player": true, "can_be_remote_controller": true, "volume_granularity": 16 },
                    "info": { "device_id": identity.id, "type": "WEB", "title": identity.title, "app_name": "Chrome" },
                    "volume_info": { "volume": 1.0 },
                    "is_shadow": false
                },
                "is_currently_active": active
            }
        }),
        Outgoing::Status(playing) => {
            json!({ "update_playing_status": { "playing_status": status(playing, versions) } })
        }
        Outgoing::Queue { queue: tracks, index, playing } => json!({
            "update_player_state": {
                "player_state": { "player_queue": queue(tracks, *index, versions), "status": status(playing, versions) }
            }
        }),
        Outgoing::Activate(device) => json!({ "update_active_device": { "device_id_optional": device } }),
        Outgoing::Volume { device, volume } => {
            json!({ "update_volume_info": { "device_id": device, "volume_info": { "volume": volume } } })
        }
    };
    message["rid"] = format!("{:x}-{}", now_ms(), identity.id).into();
    message["player_action_timestamp_ms"] = now_ms().into();
    message["activity_interception_type"] = "DO_NOT_INTERCEPT_BY_DEFAULT".into();
    message
}

pub fn parse_state(value: &Value) -> std::result::Result<Shared, (String, u64)> {
    if let Some(error) = value.get("error") {
        let wait = error.at("details").id("ynison-go-away-for-seconds").parse().unwrap_or(RETRY.as_secs());
        return Err((error.text("message").to_owned(), wait));
    }
    let state = value.at("player_state");
    let queue = state.at("player_queue");
    let status = state.at("status");
    Ok(Shared {
        devices: value
            .list("devices")
            .iter()
            .map(|device| Device {
                id: device.at("info").text("device_id").to_owned(),
                title: device.at("info").text("title").to_owned(),
                kind: device.at("info").text("type").to_owned(),
                volume: device.at("volume_info").at("volume").as_f64().unwrap_or_default(),
            })
            .collect(),
        active: Some(value.text("active_device_id_optional").to_owned()).filter(|id| !id.is_empty()),
        queue: queue
            .list("playable_list")
            .iter()
            .map(|playable| Playable { id: playable.id("playable_id"), album_id: playable.id("album_id_optional") })
            .collect(),
        index: usize::try_from(queue.at("current_playable_index").as_i64().unwrap_or(-1)).ok(),
        playing: Playing {
            paused: status.flag("paused"),
            progress_ms: status.id("progress_ms").parse().unwrap_or_default(),
            duration_ms: status.id("duration_ms").parse().unwrap_or_default(),
            speed: status.at("playback_speed").as_f64().unwrap_or(1.0),
        },
    })
}

fn protocol(identity: &Identity, extra: &[(&str, &str)]) -> String {
    let mut fields = Map::new();
    fields.insert("Ynison-Device-Id".into(), identity.id.clone().into());
    fields.insert("Ynison-Device-Info".into(), json!({ "app_name": "Chrome", "type": 1 }).to_string().into());
    for (key, value) in extra {
        fields.insert((*key).into(), (*value).into());
    }
    format!("Bearer, v2, {}", Value::Object(fields))
}

fn open(url: &str, token: &str, protocol: &str) -> Result<Socket> {
    let mut request = url.into_client_request().map_err(failure)?;
    let headers = request.headers_mut();
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_str(protocol).map_err(failure)?);
    headers.insert("Origin", HeaderValue::from_static("http://music.yandex.ru"));
    headers.insert("Authorization", HeaderValue::from_str(&format!("OAuth {token}")).map_err(failure)?);
    Ok(tungstenite::connect(request).map_err(failure)?.0)
}

fn read_json(socket: &mut Socket) -> Result<Option<Value>> {
    match socket.read() {
        Ok(Message::Text(text)) => serde_json::from_str(&text).map(Some).map_err(failure),
        Ok(Message::Close(frame)) => Err(failure(format!("closed {frame:?}"))),
        Ok(_) => Ok(None),
        Err(tungstenite::Error::Io(error))
            if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
        {
            Ok(None)
        }
        Err(error) => Err(failure(error)),
    }
}

fn connect(token: &str, identity: &Identity) -> Result<Socket> {
    let mut redirect = open(REDIRECTOR, token, &protocol(identity, &[]))?;
    let target = loop {
        if let Some(target) = read_json(&mut redirect)? {
            break target;
        }
    };
    let _ = redirect.close(None);
    let (host, ticket, session) = (target.text("host"), target.text("redirect_ticket"), target.id("session_id"));
    if host.is_empty() {
        return Err(failure(target));
    }
    let extra = [("Ynison-Redirect-Ticket", ticket), ("Ynison-Session-Id", session.as_str())];
    let socket = open(&format!("wss://{host}{STATE_PATH}"), token, &protocol(identity, &extra))?;
    if let MaybeTlsStream::NativeTls(stream) = socket.get_ref() {
        stream.get_ref().set_read_timeout(Some(READ_TIMEOUT))?;
    }
    Ok(socket)
}

pub struct Link {
    commands: Sender<Outgoing>,
    events: Receiver<Incoming>,
    stop: Arc<AtomicBool>,
}

impl Link {
    pub fn start(token: String, identity: Identity) -> Link {
        let (commands, inbox) = mpsc::channel();
        let (outbox, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        std::thread::spawn(move || run(&token, &identity, &inbox, &outbox, &running));
        Link { commands, events, stop }
    }

    pub fn send(&self, outgoing: Outgoing) {
        let _ = self.commands.send(outgoing);
    }

    pub fn take(&self) -> Vec<Incoming> {
        self.events.try_iter().collect()
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn pause(stop: &AtomicBool, seconds: u64) {
    for _ in 0..seconds * 4 {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn run(token: &str, identity: &Identity, inbox: &Receiver<Outgoing>, outbox: &Sender<Incoming>, stop: &AtomicBool) {
    let versions = Versions::new(&identity.id);
    let mut full: Option<Outgoing> = None;
    while !stop.load(Ordering::Relaxed) {
        let mut socket = match connect(token, identity) {
            Ok(socket) => socket,
            Err(error) => {
                let _ = outbox.send(Incoming::Unavailable(error.to_string()));
                pause(stop, RETRY.as_secs());
                continue;
            }
        };
        let announce = full.clone().unwrap_or(Outgoing::Full {
            queue: Vec::new(),
            index: None,
            playing: Playing { paused: true, speed: 1.0, ..Playing::default() },
            active: false,
        });
        let mut wait = None;
        let mut pending = vec![announce];
        while !stop.load(Ordering::Relaxed) && wait.is_none() {
            pending.extend(inbox.try_iter());
            for outgoing in pending.drain(..) {
                if matches!(outgoing, Outgoing::Full { .. }) {
                    full = Some(outgoing.clone());
                }
                let text = message(identity, &versions, &outgoing).to_string();
                if socket.send(Message::Text(text.into())).is_err() {
                    wait = Some(RETRY.as_secs());
                }
            }
            match read_json(&mut socket) {
                Ok(Some(value)) => match parse_state(&value) {
                    Ok(shared) => {
                        let _ = outbox.send(Incoming::State(shared));
                    }
                    Err((reason, seconds)) => {
                        let _ = outbox.send(Incoming::Unavailable(reason));
                        wait = Some(seconds.max(RETRY.as_secs()));
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    let _ = outbox.send(Incoming::Unavailable(error.to_string()));
                    wait = Some(RETRY.as_secs());
                }
            }
        }
        let _ = socket.close(None);
        pause(stop, wait.unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> Identity {
        Identity { id: "pc".into(), title: "Tactus on PC".into() }
    }

    #[test]
    fn builds_protocol_messages() {
        let versions = Versions::new("pc");
        let playing = Playing { paused: false, progress_ms: 5_000, duration_ms: 60_000, speed: 1.0 };
        let tracks = vec![Playable { id: "1".into(), album_id: "10".into() }];
        let full = message(
            &identity(),
            &versions,
            &Outgoing::Full { queue: tracks.clone(), index: Some(0), playing: playing.clone(), active: true },
        );
        let state = &full["update_full_state"];
        assert_eq!(state["device"]["info"]["title"], "Tactus on PC");
        assert_eq!(state["player_state"]["player_queue"]["playable_list"][0]["playable_id"], "1");
        assert_eq!(state["player_state"]["status"]["progress_ms"], 5_000);
        assert_eq!(full["activity_interception_type"], "DO_NOT_INTERCEPT_BY_DEFAULT");
        let first = state["player_state"]["player_queue"]["version"]["version"].as_u64().unwrap();
        let second = state["player_state"]["status"]["version"]["version"].as_u64().unwrap();
        assert!(second > first);
        let activate = message(&identity(), &versions, &Outgoing::Activate("phone".into()));
        assert_eq!(activate["update_active_device"]["device_id_optional"], "phone");
        let queue = message(&identity(), &versions, &Outgoing::Queue { queue: tracks, index: None, playing });
        assert_eq!(queue["update_player_state"]["player_state"]["player_queue"]["current_playable_index"], -1);
    }

    #[test]
    fn parses_shared_state_and_errors() {
        let value = json!({
            "player_state": {
                "player_queue": {
                    "current_playable_index": 1,
                    "playable_list": [{ "playable_id": "1", "album_id_optional": "10" }, { "playable_id": "2" }]
                },
                "status": { "paused": true, "progress_ms": "1200", "duration_ms": "60000", "playback_speed": 1.5 }
            },
            "devices": [{ "info": { "device_id": "phone", "title": "Phone", "type": "ANDROID" }, "volume_info": { "volume": 0.5 } }],
            "active_device_id_optional": "phone"
        });
        let shared = parse_state(&value).unwrap();
        assert_eq!((shared.index, shared.queue.len(), shared.active.as_deref()), (Some(1), 2, Some("phone")));
        assert_eq!((shared.playing.progress_ms, shared.playing.speed), (1200, 1.5));
        assert_eq!(shared.devices[0].title, "Phone");
        let error = json!({ "error": { "message": "Ynison service is temporary unavailable.", "details": { "ynison-go-away-for-seconds": "3600" } } });
        assert_eq!(parse_state(&error), Err(("Ynison service is temporary unavailable.".to_owned(), 3600)));
    }
}
