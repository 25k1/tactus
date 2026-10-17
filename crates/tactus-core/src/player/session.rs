use crate::settings;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use tactus_api::Track;
use tactus_api::error::Result;

const FILE: &str = "session.json";
const POSITIONS_LIMIT: usize = 200;
const SEARCHES_LIMIT: usize = 20;
const PLAYED_LIMIT: usize = 5000;
const SEEN_LIMIT: usize = 2000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub tracks: Vec<Track>,
    pub index: usize,
    pub position_ms: u64,
    pub wave: Vec<String>,
    pub positions: HashMap<String, u64>,
    pub searches: Vec<String>,
    pub played: BTreeSet<String>,
    pub seen_episodes: BTreeSet<String>,
}

impl Session {
    pub fn load() -> Session {
        settings::read_json(FILE).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        settings::write_json(FILE, self)
    }

    pub fn remember_position(&mut self, track: &Track, position_ms: u64) {
        let near_end = track.duration_ms > 0 && position_ms + 30_000 >= track.duration_ms;
        if track.episode && near_end {
            self.set_played(&track.id, true);
        }
        if !track.episode || near_end || position_ms < 10_000 {
            self.positions.remove(&track.id);
            return;
        }
        if self.positions.len() >= POSITIONS_LIMIT
            && !self.positions.contains_key(&track.id)
            && let Some(oldest) = self.positions.keys().next().cloned()
        {
            self.positions.remove(&oldest);
        }
        self.positions.insert(track.id.clone(), position_ms);
    }

    pub fn set_played(&mut self, id: &str, played: bool) {
        if !played {
            self.played.remove(id);
        } else if self.played.len() < PLAYED_LIMIT {
            self.played.insert(id.to_owned());
        }
    }

    pub fn unseen_episodes(&mut self, tracks: &[Track]) -> Vec<Track> {
        let first = self.seen_episodes.is_empty();
        let unseen: Vec<Track> =
            tracks.iter().filter(|track| track.episode && !self.seen_episodes.contains(&track.id)).cloned().collect();
        for track in &unseen {
            if self.seen_episodes.len() >= SEEN_LIMIT {
                self.seen_episodes.pop_first();
            }
            self.seen_episodes.insert(track.id.clone());
        }
        if first { Vec::new() } else { unseen }
    }

    pub fn remember_search(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        let lower = query.to_lowercase();
        self.searches.retain(|search| search.to_lowercase() != lower);
        self.searches.insert(0, query.to_owned());
        self.searches.truncate(SEARCHES_LIMIT);
    }

    pub fn resume_position(&self, track: &Track) -> u64 {
        if track.episode { self.positions.get(&track.id).copied().unwrap_or(0) } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tactus_api::fake::track;

    #[test]
    fn reports_only_episodes_not_seen_before() {
        let mut session = Session::default();
        let episode = |id: &str| Track { episode: true, ..track(id) };
        assert!(session.unseen_episodes(&[episode("1"), track("song")]).is_empty());
        let unseen = session.unseen_episodes(&[episode("1"), episode("2")]);
        assert_eq!(unseen.iter().map(|track| track.id.as_str()).collect::<Vec<_>>(), ["2"]);
        assert!(session.unseen_episodes(&[episode("2")]).is_empty());
    }

    #[test]
    fn keeps_recent_unique_searches() {
        let mut session = Session::default();
        for query in ["Metallica", "ABBA", " metallica ", ""] {
            session.remember_search(query);
        }
        assert_eq!(session.searches, ["metallica", "ABBA"]);
        for index in 0..30 {
            session.remember_search(&index.to_string());
        }
        assert_eq!((session.searches.len(), session.searches[0].as_str()), (SEARCHES_LIMIT, "29"));
    }

    #[test]
    fn remembers_episode_positions_only() {
        let mut session = Session::default();
        let mut episode = track("e");
        episode.episode = true;
        session.remember_position(&episode, 60_000);
        assert_eq!(session.resume_position(&episode), 60_000);
        session.remember_position(&episode, 175_000);
        assert_eq!(session.resume_position(&episode), 0);
        assert!(session.played.contains("e"));
        let song = track("s");
        session.remember_position(&song, 60_000);
        assert_eq!(session.resume_position(&song), 0);
    }

    #[test]
    fn session_round_trips() {
        let session = Session {
            tracks: vec![track("1")],
            index: 0,
            position_ms: 5,
            wave: vec!["user:onyourwave".into()],
            searches: vec!["abba".into()],
            played: BTreeSet::from(["e".to_owned()]),
            seen_episodes: BTreeSet::new(),
            positions: HashMap::new(),
        };
        let text = serde_json::to_string(&session).unwrap();
        assert_eq!(serde_json::from_str::<Session>(&text).unwrap(), session);
    }
}
