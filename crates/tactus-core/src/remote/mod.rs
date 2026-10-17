mod server;

use crate::player::SleepTimer;
use crate::player::queue::Repeat;
use serde::Deserialize;
use serde_json::{Value, json};
pub use server::RemoteServer;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "command", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RemoteCommand {
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    Seek { position_ms: Option<u64>, delta_ms: Option<i64> },
    Volume { percent: Option<i16>, delta: Option<i16> },
    Speed { rate: Option<f32>, delta: Option<f32> },
    Repeat { mode: Option<Repeat> },
    Shuffle { enabled: Option<bool> },
    Sleep { timer: Option<SleepTimer> },
    Like,
    Unlike,
    ToggleLike,
    Dislike,
    PlayTrack { track_id: String },
    PlayAlbum { album_id: String },
    PlayArtist { artist_id: String },
    PlayPlaylist { owner_uid: u64, kind: u32 },
    StartWave { seeds: Option<String> },
    PlayIndex { index: usize },
    RemoveFromQueue { index: usize },
    MoveInQueue { from: usize, to: usize },
    ClearQueue,
    Undo,
    Voice { text: String },
    Search { query: String },
    Download,
    SpeakNowPlaying,
    Show,
    Hide,
}

#[derive(Default)]
struct Snapshot {
    status: Value,
    queue: Value,
    lyrics: Value,
}

pub struct RemoteHub {
    snapshot: Mutex<Snapshot>,
    clients: Mutex<Vec<Sender<String>>>,
    commands: Sender<RemoteCommand>,
}

impl RemoteHub {
    pub fn new() -> (Arc<RemoteHub>, Receiver<RemoteCommand>) {
        let (commands, receiver) = mpsc::channel();
        let hub = RemoteHub {
            snapshot: Mutex::new(Snapshot { status: json!({}), queue: json!([]), lyrics: json!(null) }),
            clients: Mutex::new(Vec::new()),
            commands,
        };
        (Arc::new(hub), receiver)
    }

    fn snapshot(&self) -> MutexGuard<'_, Snapshot> {
        self.snapshot.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn broadcast(&self, kind: &str, data: Value) {
        let message = json!({ "type": kind, "data": data }).to_string();
        self.clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|client| client.send(message.clone()).is_ok());
    }

    pub fn publish_status(&self, status: Value) {
        let changed = {
            let mut snapshot = self.snapshot();
            let changed = snapshot.status != status;
            snapshot.status = status.clone();
            changed
        };
        if changed {
            self.broadcast("status", status);
        }
    }

    pub fn publish_queue(&self, queue: Value) {
        self.snapshot().queue = queue.clone();
        self.broadcast("queue", queue);
    }

    pub fn publish_lyrics(&self, lyrics: Value) {
        self.snapshot().lyrics = lyrics.clone();
        self.broadcast("lyrics", lyrics);
    }

    fn get(&self, what: &str) -> Option<Value> {
        let snapshot = self.snapshot();
        Some(match what {
            "status" => snapshot.status.clone(),
            "queue" => snapshot.queue.clone(),
            "lyrics" => snapshot.lyrics.clone(),
            _ => return None,
        })
    }

    fn subscribe(&self) -> Receiver<String> {
        let (sender, receiver) = mpsc::channel();
        let _ = sender.send(json!({ "type": "status", "data": self.snapshot().status }).to_string());
        self.clients.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(sender);
        receiver
    }

    fn submit(&self, value: Value) -> Result<(), String> {
        let command: RemoteCommand = serde_json::from_value(value).map_err(|error| error.to_string())?;
        self.commands.send(command).map_err(|_| "Tactus is shutting down".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands() {
        let parse = |text: &str| serde_json::from_str::<RemoteCommand>(text).unwrap();
        assert_eq!(parse(r#"{"command":"next"}"#), RemoteCommand::Next);
        assert_eq!(
            parse(r#"{"command":"seek","positionMs":3000}"#),
            RemoteCommand::Seek { position_ms: Some(3000), delta_ms: None }
        );
        assert_eq!(parse(r#"{"command":"repeat","mode":"one"}"#), RemoteCommand::Repeat { mode: Some(Repeat::One) });
        assert_eq!(
            parse(r#"{"command":"playPlaylist","ownerUid":5,"kind":3}"#),
            RemoteCommand::PlayPlaylist { owner_uid: 5, kind: 3 }
        );
        assert_eq!(
            parse(r#"{"command":"sleep","timer":{"minutes":30}}"#),
            RemoteCommand::Sleep { timer: Some(SleepTimer::Minutes(30)) }
        );
        assert_eq!(
            parse(r#"{"command":"sleep","timer":"endOfTrack"}"#),
            RemoteCommand::Sleep { timer: Some(SleepTimer::EndOfTrack) }
        );
        assert_eq!(parse(r#"{"command":"sleep"}"#), RemoteCommand::Sleep { timer: None });
        assert!(serde_json::from_str::<RemoteCommand>(r#"{"command":"explode"}"#).is_err());
    }

    #[test]
    fn broadcasts_to_subscribers() {
        let (hub, _) = RemoteHub::new();
        let client = hub.subscribe();
        assert!(client.recv().unwrap().contains("status"));
        hub.publish_status(json!({"title": "Song"}));
        hub.publish_status(json!({"title": "Song"}));
        assert!(client.recv().unwrap().contains("Song"));
        assert!(client.try_recv().is_err());
    }
}
