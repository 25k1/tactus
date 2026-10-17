use super::view_model::{self, download_state};
use super::*;
use serde_json::{Value, json};
use tactus_api::{PlaylistKey, Relation, Target, View};
use tactus_core::player::{MY_WAVE, wave};
use tactus_core::remote::RemoteCommand;

fn track_json(track: &tactus_api::Track) -> Value {
    json!({
        "id": track.id,
        "title": track.title,
        "version": track.version,
        "artist": track.artist,
        "artistId": track.artist_id,
        "album": track.album,
        "albumId": track.album_id,
        "coverUrl": track.cover,
        "durationMs": track.duration_ms,
    })
}

impl Ui {
    fn current_target(&self) -> Option<Target> {
        self.player.borrow().track.clone().map(Target::Track)
    }

    pub(super) fn handle_remote(self: &Rc<Self>, command: RemoteCommand) {
        match command {
            RemoteCommand::Play => self.act(|player| player.set_playing(true)),
            RemoteCommand::Pause => self.act(|player| player.set_playing(false)),
            RemoteCommand::Toggle => self.execute(Command::PlayPause),
            RemoteCommand::Stop => self.execute(Command::Stop),
            RemoteCommand::Next => self.execute(Command::Next),
            RemoteCommand::Previous => self.execute(Command::Previous),
            RemoteCommand::Seek { position_ms, delta_ms } => self.act(|player| match (position_ms, delta_ms) {
                (Some(position), _) => player.seek(position),
                (None, Some(delta)) => player.seek_by(delta),
                (None, None) => {}
            }),
            RemoteCommand::Volume { percent, delta } => self.act(|player| match (percent, delta) {
                (Some(percent), _) => player.set_volume(percent),
                (None, Some(delta)) => player.change_volume(delta),
                (None, None) => {}
            }),
            RemoteCommand::Speed { rate, delta } => self.act(|player| match (rate, delta) {
                (Some(rate), _) => player.set_speed(rate),
                (None, Some(delta)) => player.change_speed(delta),
                (None, None) => {}
            }),
            RemoteCommand::Repeat { mode } => {
                self.act(|player| player.set_repeat(mode.unwrap_or_else(|| player.settings.repeat.cycle())));
            }
            RemoteCommand::Sleep { timer } => self.act(|player| player.set_sleep(timer)),
            RemoteCommand::Shuffle { enabled } => {
                self.act(|player| player.set_shuffle(enabled.unwrap_or(!player.settings.shuffle)));
            }
            RemoteCommand::Like | RemoteCommand::Unlike | RemoteCommand::ToggleLike | RemoteCommand::Dislike => {
                let Some(target) = self.current_target() else { return };
                self.act(|player| match command {
                    RemoteCommand::Like => player.set_relation(target, Relation::Like, true),
                    RemoteCommand::Unlike => player.set_relation(target, Relation::Like, false),
                    RemoteCommand::Dislike => player.set_relation(target, Relation::Dislike, true),
                    _ => player.toggle_relation(target, Relation::Like),
                });
            }
            RemoteCommand::PlayTrack { track_id } => self.act(|player| player.play_ids(vec![track_id])),
            RemoteCommand::PlayAlbum { album_id } => self.act(|player| player.play_view(View::Album(album_id))),
            RemoteCommand::PlayArtist { artist_id } => self.act(|player| player.play_view(View::Artist(artist_id))),
            RemoteCommand::PlayPlaylist { owner_uid, kind } => {
                let key = PlaylistKey { owner: owner_uid, kind };
                self.act(|player| player.play_view(View::Playlist(key)));
            }
            RemoteCommand::StartWave { seeds } => {
                let seeds = seeds.map(|seeds| wave::seeds(&seeds)).filter(|seeds| !seeds.is_empty());
                self.act(|player| player.start_wave(seeds.unwrap_or_else(|| vec![MY_WAVE.to_owned()])));
            }
            RemoteCommand::PlayIndex { index } => self.act(|player| player.play_index(index)),
            RemoteCommand::RemoveFromQueue { index } => self.act(|player| player.remove_from_queue(&[index])),
            RemoteCommand::Undo => self.act(Player::undo),
            RemoteCommand::Voice { text } => self.voice_text(&text),
            RemoteCommand::MoveInQueue { from, to } => self.act(|player| player.move_in_queue(from, to)),
            RemoteCommand::ClearQueue => self.act(Player::clear_queue),
            RemoteCommand::Search { query } => {
                self.browser.search.change_value(&query);
                self.run_remote_search();
            }
            RemoteCommand::Download => self.execute(Command::Download),
            RemoteCommand::SpeakNowPlaying => self.execute(Command::SpeakNowPlaying),
            RemoteCommand::Show => self.raise_window(),
            RemoteCommand::Hide => self.hide_window(),
        }
    }

    fn run_remote_search(self: &Rc<Self>) {
        let query = self.browser.search.get_value();
        self.navigate(Location::Section(Section::Search), false);
        self.act(|player| player.load(&View::Search(query.trim().to_owned()), true));
        self.render(Some(0), Some(0));
    }

    fn serving(&self) -> bool {
        self.remote_server.borrow().is_some()
    }

    pub(super) fn publish_status(&self) {
        if !self.serving() {
            return;
        }
        let player = self.player.borrow();
        let target = player.track.clone().map(Target::Track);
        let has = |relation| target.as_ref().is_some_and(|target| player.has(target, relation));
        let mut status = json!({
            "playing": player.playing,
            "positionMs": player.position_ms,
            "volume": player.settings.volume,
            "speed": player.speed(),
            "repeat": player.settings.repeat,
            "shuffle": player.settings.shuffle,
            "liked": has(Relation::Like),
            "disliked": has(Relation::Dislike),
            "queueIndex": player.queue.index(),
            "queueLength": player.queue.len(),
            "account": player.account.as_ref().map(|account| account.name.clone()),
            "nowPlaying": view_model::now_playing(&player),
            "sleep": player.sleep_timer(),
            "sleepRemainingMs": player.sleep_remaining().map(|remaining| remaining.as_millis() as u64),
        });
        status["track"] = player.track.as_ref().map_or(Value::Null, track_json);
        self.remote_hub.publish_status(status);
    }

    pub(super) fn publish_queue(&self) {
        if self.serving() {
            let queue: Vec<Value> = self.player.borrow().queue.tracks().iter().map(track_json).collect();
            self.remote_hub.publish_queue(json!(queue));
        }
    }

    pub(super) fn publish_lyrics(&self) {
        if !self.serving() {
            return;
        }
        let player = self.player.borrow();
        let lyrics = player.lyrics.as_ref().map(|lyrics| {
            json!({
                "synced": lyrics.synced(),
                "writers": lyrics.writers,
                "lines": lyrics.lines.iter().map(|line| json!({ "timeMs": line.time_ms, "text": line.text })).collect::<Vec<_>>(),
            })
        });
        self.remote_hub.publish_lyrics(lyrics.unwrap_or(Value::Null));
    }

    pub(super) fn publish_downloads(&self) {
        if !self.serving() {
            return;
        }
        let downloads: Vec<Value> = self
            .player
            .borrow()
            .downloads
            .iter()
            .map(|download| {
                json!({
                    "id": download.id,
                    "track": track_json(&download.track),
                    "state": download_state(&download.state),
                    "path": download.file().map(|path| path.display().to_string()),
                })
            })
            .collect();
        self.remote_hub.broadcast("downloads", json!(downloads));
    }
}
