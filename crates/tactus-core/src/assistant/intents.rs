use super::names::{choose, play};
use super::*;
use crate::player::queue::Repeat;
use crate::player::{MY_WAVE, SPEED_STEP, SleepTimer, VOLUME_STEP, wave};
use tactus_api::{Relation, Target};
use tactus_voice::{Mood, Pick, SleepRequest};

pub(super) fn mood_seed(mood: Mood) -> &'static str {
    let [active, fun, calm, sad] = wave::MOODS;
    match mood {
        Mood::Energetic => active,
        Mood::Fun => fun,
        Mood::Calm => calm,
        Mood::Sad => sad,
    }
}

impl Assistant {
    pub(super) fn run(&mut self, player: &mut Player, intent: Intent, now: Instant) -> Option<Outcome> {
        let feedback = |kind| Some(Outcome::Feedback(kind));
        match intent {
            Intent::Play | Intent::Pause => {
                player.set_playing(intent == Intent::Play);
                None
            }
            Intent::Stop => {
                player.stop();
                None
            }
            Intent::Next => {
                player.next();
                None
            }
            Intent::Previous => {
                player.previous();
                None
            }
            Intent::VolumeUp | Intent::VolumeDown => {
                player.change_volume(if intent == Intent::VolumeUp { VOLUME_STEP } else { -VOLUME_STEP });
                feedback(Announcement::Volume)
            }
            Intent::Volume(volume) => {
                player.set_volume(i16::from(volume));
                feedback(Announcement::Volume)
            }
            Intent::Faster => {
                player.change_speed(SPEED_STEP);
                Some(Outcome::Speed)
            }
            Intent::Slower => {
                player.change_speed(-SPEED_STEP);
                Some(Outcome::Speed)
            }
            Intent::NormalSpeed => {
                player.set_speed(1.0);
                Some(Outcome::Speed)
            }
            Intent::Forward(seconds) | Intent::Rewind(seconds) => {
                let step = i64::from(seconds) * 1000;
                player.seek_by(if matches!(intent, Intent::Forward(_)) { step } else { -step });
                feedback(Announcement::Seek)
            }
            Intent::RepeatOff | Intent::RepeatOne | Intent::RepeatAll => {
                player.set_repeat(match intent {
                    Intent::RepeatOne => Repeat::One,
                    Intent::RepeatAll => Repeat::All,
                    _ => Repeat::Off,
                });
                feedback(Announcement::Modes)
            }
            Intent::ShuffleOn | Intent::ShuffleOff => {
                player.set_shuffle(intent == Intent::ShuffleOn);
                feedback(Announcement::Modes)
            }
            Intent::Sleep(request) => {
                player.set_sleep(match request {
                    SleepRequest::Minutes(minutes) => Some(SleepTimer::Minutes(minutes)),
                    SleepRequest::EndOfTrack => Some(SleepTimer::EndOfTrack),
                    SleepRequest::Off => None,
                });
                None
            }
            Intent::PlayWave => {
                player.start_wave(vec![MY_WAVE.to_owned()]);
                None
            }
            Intent::PlayMood(mood) => {
                player.start_wave(vec![MY_WAVE.to_owned(), mood_seed(mood).to_owned()]);
                None
            }
            Intent::PlayName { kind, name } => Some(self.play_name(player, kind, name, now)),
            Intent::PlayLikes => self.play_likes(player),
            Intent::Like => relate(player, Relation::Like, true),
            Intent::Unlike => relate(player, Relation::Like, false),
            Intent::Dislike => relate(player, Relation::Dislike, true),
            Intent::AddToPlaylist(name) => add_to_playlist(player, name),
            Intent::Yes => {
                if let Some((item, _)) = self.pending.take() {
                    play(player, item);
                }
                None
            }
            Intent::No => self.pending.take().map(|_| Outcome::Reply(Reply::Cancelled)),
            Intent::WhatsPlaying => Some(Outcome::WhatsPlaying),
            Intent::Download => Some(Outcome::Download),
            Intent::GoToAlbum => Some(Outcome::GoToAlbum),
            Intent::GoToArtist => Some(Outcome::GoToArtist),
            Intent::Open(place) => Some(Outcome::Open(place)),
            Intent::Search(text) => Some(Outcome::Search(text)),
            Intent::ReadSelection => Some(Outcome::ReadSelection),
            Intent::GoBack => Some(Outcome::GoBack),
            Intent::Help => Some(Outcome::Help),
        }
    }
}

fn relate(player: &mut Player, relation: Relation, on: bool) -> Option<Outcome> {
    let Some(track) = player.track.clone() else {
        return Some(Outcome::Reply(Reply::NothingPlaying));
    };
    player.set_relation(Target::Track(track), relation, on);
    None
}

fn add_to_playlist(player: &mut Player, name: String) -> Option<Outcome> {
    let Some(track) = player.track.clone() else {
        return Some(Outcome::Reply(Reply::NothingPlaying));
    };
    let mut own: Vec<_> = player.playlists.iter().filter(|playlist| player.is_own(playlist)).cloned().collect();
    match choose(&name, own.iter().map(|playlist| playlist.title.clone())) {
        Pick::One(index) => {
            player.add_to_playlist(&own.swap_remove(index), vec![track]);
            None
        }
        _ => Some(Outcome::Reply(Reply::NoPlaylist(name))),
    }
}
