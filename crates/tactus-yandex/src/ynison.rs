use crate::parse::Json;
use serde_json::{Map, Value, json};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tactus_api::error::{Error, Result};
use tactus_api::proxy::encode_component;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const REDIRECTOR: &str = "wss://ynison.music.yandex.ru/redirector.YnisonRedirectService/GetRedirectToYnison";
const STATE_PATH: &str = "/ynison_state.YnisonStateService/PutYnisonState";
const APP_NAME: &str = "yandex-music";
const FROM: &str = "desktop_win-own_tracks-track-default";
const READ_TIMEOUT: Duration = Duration::from_millis(250);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(20);
const PING_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_BACKOFF_MS: [u64; 4] = [0, 1000, 5000, 30000];
const JITTER_MS: u64 = 250;
const GRPC_UNAUTHENTICATED: i64 = 16;
const GRPC_PERMISSION_DENIED: i64 = 7;
const DEVICE_DISPLACED: &str = "400090001";

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
                    "info": { "device_id": identity.id, "type": "WEB", "title": identity.title, "app_name": APP_NAME },
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerError {
    pub message: String,
    pub grpc_code: Option<i64>,
    pub http_code: Option<i64>,
    pub error_code: String,
    pub backoff_ms: Vec<u64>,
    pub go_away_seconds: Option<u64>,
}

impl ServerError {
    fn parse(error: &Value) -> Self {
        let details =
            Some(error.at("details")).filter(|details| details.as_object().is_some_and(|map| !map.is_empty()));
        let details = details.unwrap_or_else(|| error.at("extra_headers"));
        let number = |value: &Value| value.as_i64().or_else(|| value.as_str()?.trim().parse().ok());
        Self {
            message: Some(error.text("message"))
                .filter(|message| !message.is_empty())
                .unwrap_or("Ynison error")
                .to_owned(),
            grpc_code: number(error.at("grpc_code")),
            http_code: number(error.at("http_code")),
            error_code: details.id("ynison-error-code"),
            backoff_ms: details
                .text("ynison-backoff-millis")
                .split(':')
                .filter_map(|part| part.trim().parse().ok())
                .collect(),
            go_away_seconds: details.id("ynison-go-away-for-seconds").parse().ok().filter(|seconds| *seconds > 0),
        }
    }

    pub fn unauthorized(&self) -> bool {
        matches!(self.grpc_code, Some(GRPC_UNAUTHENTICATED | GRPC_PERMISSION_DENIED))
            || matches!(self.http_code, Some(401 | 403))
    }

    pub fn displaced(&self) -> bool {
        self.error_code == DEVICE_DISPLACED
    }
}

pub fn parse_state(value: &Value) -> std::result::Result<Shared, ServerError> {
    if let Some(error) = value.get("error") {
        return Err(ServerError::parse(error));
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

pub fn protocol(identity: &Identity, extra: &[(&str, &str)]) -> String {
    let mut fields = Map::new();
    fields.insert("Ynison-Device-Id".into(), identity.id.clone().into());
    fields.insert("Ynison-Device-Info".into(), json!({ "app_name": identity.title, "type": "1" }).to_string().into());
    for (key, value) in extra {
        fields.insert((*key).into(), (*value).into());
    }
    format!("Bearer, v2, {}", encode_component(&Value::Object(fields).to_string()))
}

enum Failure {
    Retry(String),
    Server(ServerError),
    Unauthorized(String),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Failure::Retry(error.to_string())
    }
}

fn open(url: &str, token: &str, protocol: &str) -> std::result::Result<Socket, Failure> {
    let mut request = url.into_client_request().map_err(failure)?;
    let headers = request.headers_mut();
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_str(protocol).map_err(failure)?);
    headers.insert("Origin", HeaderValue::from_static("https://music.yandex.ru"));
    headers.insert("Authorization", HeaderValue::from_str(&format!("OAuth {token}")).map_err(failure)?);
    let host = request.uri().host().ok_or_else(|| failure("no host"))?.to_owned();
    let stream = tactus_api::net::open_stream(&host, 443).map_err(failure)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)).map_err(failure)?;
    match tungstenite::client_tls(request, stream) {
        Ok((socket, _)) => Ok(socket),
        Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response)))
            if matches!(response.status().as_u16(), 401 | 403) =>
        {
            Err(Failure::Unauthorized(format!("ynison: HTTP {}", response.status())))
        }
        Err(error) => Err(Failure::Retry(failure(error).to_string())),
    }
}

enum Frame {
    Json(Value),
    Control,
    Idle,
}

fn read_frame(socket: &mut Socket) -> Result<Frame> {
    match socket.read() {
        Ok(Message::Text(text)) => serde_json::from_str(&text).map(Frame::Json).map_err(failure),
        Ok(Message::Close(frame)) => Err(failure(format!("closed {frame:?}"))),
        Ok(_) => Ok(Frame::Control),
        Err(tungstenite::Error::Io(error))
            if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
        {
            Ok(Frame::Idle)
        }
        Err(error) => Err(failure(error)),
    }
}

fn set_read_timeout(socket: &Socket, timeout: Duration) -> Result<()> {
    if let MaybeTlsStream::NativeTls(stream) = socket.get_ref() {
        stream.get_ref().set_read_timeout(Some(timeout))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeepAlive {
    pub interval: Duration,
    pub timeout: Duration,
}

impl KeepAlive {
    fn parse(params: &Value) -> Self {
        let seconds = |key: &str, default: Duration| {
            params.id(key).parse::<u64>().ok().filter(|seconds| *seconds > 0).map_or(default, Duration::from_secs)
        };
        Self {
            interval: seconds("keep_alive_time_seconds", PING_INTERVAL),
            timeout: seconds("keep_alive_timeout_seconds", PING_TIMEOUT),
        }
    }
}

fn connect(token: &str, identity: &Identity) -> std::result::Result<(Socket, KeepAlive), Failure> {
    let mut redirect = open(REDIRECTOR, token, &protocol(identity, &[]))?;
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    let target = loop {
        match read_frame(&mut redirect)? {
            Frame::Json(target) => break target,
            _ if Instant::now() >= deadline => return Err(Failure::Retry("ynison: no redirect".into())),
            _ => {}
        }
    };
    let _ = redirect.close(None);
    if let Some(error) = target.get("error") {
        return Err(Failure::Server(ServerError::parse(error)));
    }
    let (host, ticket, session) = (target.text("host"), target.text("redirect_ticket"), target.id("session_id"));
    if host.is_empty() || ticket.is_empty() || session.is_empty() {
        return Err(Failure::Retry(failure(target).to_string()));
    }
    let extra = [("Ynison-Redirect-Ticket", ticket), ("Ynison-Session-Id", session.as_str())];
    let socket = open(&format!("wss://{host}{STATE_PATH}"), token, &protocol(identity, &extra))?;
    set_read_timeout(&socket, READ_TIMEOUT)?;
    Ok((socket, KeepAlive::parse(target.at("keep_alive_params"))))
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

fn pause(stop: &AtomicBool, wait: Duration) {
    let deadline = Instant::now() + wait;
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100).min(deadline - Instant::now()));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backoff {
    schedule: Vec<u64>,
    attempt: usize,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { schedule: DEFAULT_BACKOFF_MS.to_vec(), attempt: 0 }
    }
}

impl Backoff {
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn adopt(&mut self, schedule: &[u64]) {
        if !schedule.is_empty() {
            self.schedule = schedule.to_vec();
        }
    }

    pub fn delay(&mut self) -> Duration {
        self.attempt += 1;
        let base = self.schedule[(self.attempt - 1).min(self.schedule.len() - 1)];
        Duration::from_millis(base + now_ms() % JITTER_MS)
    }
}

enum Verdict {
    Wait(Duration),
    Stop,
}

fn judge(error: &ServerError, backoff: &mut Backoff) -> Verdict {
    backoff.adopt(&error.backoff_ms);
    if error.unauthorized() || error.displaced() {
        return Verdict::Stop;
    }
    match error.go_away_seconds {
        Some(seconds) => Verdict::Wait(Duration::from_secs(seconds)),
        None => Verdict::Wait(backoff.delay()),
    }
}

struct Context<'a> {
    identity: &'a Identity,
    versions: Versions,
    inbox: &'a Receiver<Outgoing>,
    outbox: &'a Sender<Incoming>,
    stop: &'a AtomicBool,
}

fn session(
    socket: &mut Socket,
    keep_alive: KeepAlive,
    context: &Context<'_>,
    full: &mut Option<Outgoing>,
    backoff: &mut Backoff,
) -> std::result::Result<(), Failure> {
    let Context { identity, versions, inbox, outbox, stop } = context;
    let announce = full.clone().unwrap_or(Outgoing::Full {
        queue: Vec::new(),
        index: None,
        playing: Playing { paused: true, speed: 1.0, ..Playing::default() },
        active: false,
    });
    let mut pending = vec![announce];
    let (mut heard, mut pinged) = (Instant::now(), Instant::now());
    while !stop.load(Ordering::Relaxed) {
        pending.extend(inbox.try_iter());
        for outgoing in pending.drain(..) {
            if matches!(outgoing, Outgoing::Full { .. }) {
                *full = Some(outgoing.clone());
            }
            let text = message(identity, versions, &outgoing).to_string();
            socket.send(Message::Text(text.into())).map_err(failure)?;
        }
        if pinged.elapsed() >= keep_alive.interval {
            socket.send(Message::Ping(Vec::new().into())).map_err(failure)?;
            pinged = Instant::now();
        }
        match read_frame(socket)? {
            Frame::Json(value) => {
                heard = Instant::now();
                let shared = parse_state(&value).map_err(Failure::Server)?;
                backoff.reset();
                let _ = outbox.send(Incoming::State(shared));
            }
            Frame::Control => heard = Instant::now(),
            Frame::Idle if heard.elapsed() >= keep_alive.interval + keep_alive.timeout => {
                return Err(Failure::Retry("ynison: the server stopped answering".into()));
            }
            Frame::Idle => {}
        }
    }
    Ok(())
}

fn run(token: &str, identity: &Identity, inbox: &Receiver<Outgoing>, outbox: &Sender<Incoming>, stop: &AtomicBool) {
    let context = Context { identity, versions: Versions::new(&identity.id), inbox, outbox, stop };
    let mut full: Option<Outgoing> = None;
    let mut backoff = Backoff::default();
    while !stop.load(Ordering::Relaxed) {
        let result = connect(token, identity).and_then(|(mut socket, keep_alive)| {
            let result = session(&mut socket, keep_alive, &context, &mut full, &mut backoff);
            let _ = socket.close(None);
            result
        });
        let verdict = match result {
            Ok(()) => return,
            Err(Failure::Retry(reason)) => {
                let _ = outbox.send(Incoming::Unavailable(reason));
                Verdict::Wait(backoff.delay())
            }
            Err(Failure::Unauthorized(reason)) => {
                let _ = outbox.send(Incoming::Unavailable(reason));
                Verdict::Stop
            }
            Err(Failure::Server(error)) => {
                let _ = outbox.send(Incoming::Unavailable(error.message.clone()));
                judge(&error, &mut backoff)
            }
        };
        match verdict {
            Verdict::Wait(wait) => pause(stop, wait),
            Verdict::Stop => return,
        }
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
        let error = json!({ "error": {
            "message": "Ynison service is temporary unavailable.",
            "grpc_code": 14,
            "http_code": 503,
            "details": { "ynison-error-code": "500140000", "ynison-go-away-for-seconds": "3600", "ynison-backoff-millis": "0:2000:9000" }
        } });
        let error = parse_state(&error).unwrap_err();
        assert_eq!(error.message, "Ynison service is temporary unavailable.");
        assert_eq!((error.grpc_code, error.http_code, error.go_away_seconds), (Some(14), Some(503), Some(3600)));
        assert_eq!(error.backoff_ms, vec![0, 2000, 9000]);
        assert!(!error.unauthorized() && !error.displaced());
    }

    #[test]
    fn classifies_server_errors() {
        let parse = |error: Value| ServerError::parse(&error);
        assert!(parse(json!({ "grpc_code": "16" })).unauthorized());
        assert!(parse(json!({ "http_code": 403 })).unauthorized());
        assert!(parse(json!({ "extra_headers": { "ynison-error-code": "400090001" } })).displaced());
        assert_eq!(parse(json!({})).message, "Ynison error");
        let mut backoff = Backoff::default();
        assert!(matches!(judge(&parse(json!({ "grpc_code": 16 })), &mut backoff), Verdict::Stop));
        let go_away = parse(json!({ "details": { "ynison-go-away-for-seconds": "60" } }));
        assert!(matches!(judge(&go_away, &mut backoff), Verdict::Wait(wait) if wait == Duration::from_secs(60)));
    }

    #[test]
    fn backs_off_on_the_server_schedule() {
        let mut backoff = Backoff::default();
        let steps: Vec<u64> = (0..5).map(|_| backoff.delay().as_millis() as u64 / 1000).collect();
        assert_eq!(steps, vec![0, 1, 5, 30, 30]);
        backoff.adopt(&[100, 200]);
        backoff.reset();
        assert!(backoff.delay() < Duration::from_millis(100 + JITTER_MS));
        assert!(backoff.delay() >= Duration::from_millis(200));
    }

    #[test]
    fn encodes_the_handshake_like_the_official_client() {
        let encoded = protocol(&identity(), &[("Ynison-Session-Id", "42")]);
        let rest = encoded.strip_prefix("Bearer, v2, ").unwrap();
        assert!(rest.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"%-_.~".contains(&byte)));
        assert!(rest.contains("Ynison-Session-Id%22%3A%2242%22"));
        assert!(rest.contains("%5C%22app_name%5C%22%3A%5C%22Tactus%20on%20PC%5C%22"));
        assert!(rest.contains("%5C%22type%5C%22%3A%5C%221%5C%22"));
        let keep_alive = KeepAlive::parse(&json!({ "keep_alive_time_seconds": 5, "keep_alive_timeout_seconds": "7" }));
        assert_eq!(keep_alive, KeepAlive { interval: Duration::from_secs(5), timeout: Duration::from_secs(7) });
        assert_eq!(KeepAlive::parse(&Value::Null).interval, PING_INTERVAL);
    }
}
