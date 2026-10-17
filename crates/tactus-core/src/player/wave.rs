use std::collections::{HashMap, HashSet};
use tactus_api::{Track, WaveBatch};

pub const MY_WAVE: &str = "user:onyourwave";
pub const QUEUE_LIMIT: usize = 200;
pub const CHARACTERS: [&str; 3] =
    ["settingDiversity:favorite", "settingDiversity:discover", "settingDiversity:popular"];
pub const MOODS: [&str; 4] =
    ["settingMoodEnergy:active", "settingMoodEnergy:fun", "settingMoodEnergy:calm", "settingMoodEnergy:sad"];
pub const LANGUAGES: [&str; 3] =
    ["settingLanguage:russian", "settingLanguage:not-russian", "settingLanguage:without-words"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wave {
    pub seeds: Vec<String>,
    pub session: String,
    pub fetching: bool,
    pub play_when_ready: bool,
    batch: String,
    batches: HashMap<String, String>,
    seen: HashSet<String>,
}

impl Wave {
    pub fn is_active(&self) -> bool {
        !self.session.is_empty()
    }

    pub fn begin(&mut self, seeds: Vec<String>, session: String) {
        *self = Wave { seeds, session, ..Wave::default() };
    }

    pub fn clear(&mut self) {
        *self = Wave::default();
    }

    pub fn accept(&mut self, batch: WaveBatch, disliked: &HashSet<String>) -> Vec<Track> {
        self.fetching = false;
        if batch.session != self.session {
            return Vec::new();
        }
        self.batch = batch.batch.clone();
        batch
            .tracks
            .into_iter()
            .filter(|track| !disliked.contains(&track.id) && self.seen.insert(track.id.clone()))
            .inspect(|track| {
                self.batches.insert(track.id.clone(), batch.batch.clone());
            })
            .collect()
    }

    pub fn batch_of(&self, track_id: &str) -> String {
        self.batches.get(track_id).cloned().unwrap_or_else(|| self.batch.clone())
    }

    pub fn contains(&self, track_id: &str) -> bool {
        self.batches.contains_key(track_id)
    }
}

pub fn seeds(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|seed| !seed.is_empty()).map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tactus_api::fake::tracks;

    fn batch(session: &str, batch: &str, ids: &[&str]) -> WaveBatch {
        WaveBatch { session: session.into(), batch: batch.into(), tracks: tracks(ids) }
    }

    #[test]
    fn accepts_new_tracks_once_and_skips_dislikes() {
        let mut wave = Wave::default();
        wave.begin(vec![MY_WAVE.into()], "s".into());
        let disliked: HashSet<String> = ["b".to_owned()].into();
        let accepted = wave.accept(batch("s", "1", &["a", "b", "c"]), &disliked);
        assert_eq!(accepted.len(), 2);
        assert!(wave.accept(batch("s", "2", &["a", "c"]), &disliked).is_empty());
        assert_eq!(wave.batch_of("a"), "1");
        assert_eq!(wave.batch_of("unknown"), "2");
        assert!(wave.accept(batch("other", "3", &["d"]), &disliked).is_empty());
    }

    #[test]
    fn splits_seeds() {
        assert_eq!(seeds(" user:onyourwave, genre:rock ,"), ["user:onyourwave", "genre:rock"]);
    }
}
