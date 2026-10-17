use serde::{Deserialize, Serialize};
use tactus_api::Track;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

impl Repeat {
    pub const ALL: [Repeat; 3] = [Repeat::Off, Repeat::One, Repeat::All];

    pub fn cycle(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Queue {
    tracks: Vec<Track>,
    index: usize,
    unshuffled: Option<Vec<Track>>,
}

fn shuffle(tracks: &mut [Track], mut seed: u64) {
    for index in (1..tracks.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        tracks.swap(index, (seed % (index as u64 + 1)) as usize);
    }
}

impl Queue {
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn current(&self) -> Option<&Track> {
        self.tracks.get(self.index)
    }

    pub fn get(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    pub fn ids(&self) -> Vec<String> {
        self.tracks.iter().map(|track| track.id.clone()).collect()
    }

    pub fn remaining(&self) -> usize {
        self.tracks.len().saturating_sub(self.index + 1)
    }

    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) {
        self.index = start.min(tracks.len().saturating_sub(1));
        self.tracks = tracks;
        self.unshuffled = None;
    }

    pub fn clear(&mut self) {
        self.replace(Vec::new(), 0);
    }

    pub fn select(&mut self, index: usize) -> Option<&Track> {
        if index < self.tracks.len() {
            self.index = index;
        }
        self.tracks.get(index)
    }

    pub fn following(&self, repeat: Repeat, manual: bool) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        match repeat {
            Repeat::One if !manual => Some(self.index),
            _ if self.index + 1 < self.tracks.len() => Some(self.index + 1),
            Repeat::All | Repeat::One => Some(0),
            Repeat::Off => None,
        }
    }

    pub fn preceding(&self, repeat: Repeat) -> Option<usize> {
        match (self.index, repeat) {
            _ if self.tracks.is_empty() => None,
            (0, Repeat::All) => Some(self.tracks.len() - 1),
            (0, _) => Some(0),
            (index, _) => Some(index - 1),
        }
    }

    fn track_original(&mut self, tracks: &[Track]) {
        if let Some(original) = self.unshuffled.as_mut() {
            original.extend_from_slice(tracks);
        }
    }

    pub fn insert_next(&mut self, tracks: Vec<Track>) {
        self.track_original(&tracks);
        let at = if self.tracks.is_empty() { 0 } else { self.index + 1 };
        self.tracks.splice(at..at, tracks);
    }

    pub fn insert_at(&mut self, index: usize, track: Track) {
        let index = index.min(self.tracks.len());
        self.track_original(std::slice::from_ref(&track));
        if !self.tracks.is_empty() && index <= self.index {
            self.index += 1;
        }
        self.tracks.insert(index, track);
    }

    pub fn append(&mut self, tracks: Vec<Track>) {
        self.track_original(&tracks);
        self.tracks.extend(tracks);
    }

    pub fn remove(&mut self, index: usize) -> Option<Track> {
        if index >= self.tracks.len() {
            return None;
        }
        let track = self.tracks.remove(index);
        if index < self.index || (index == self.index && self.index >= self.tracks.len() && self.index > 0) {
            self.index -= 1;
        }
        if let Some(original) = self.unshuffled.as_mut()
            && let Some(position) = original.iter().position(|candidate| candidate.id == track.id)
        {
            original.remove(position);
        }
        Some(track)
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tracks.len() || to >= self.tracks.len() || from == to {
            return false;
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        self.index = match self.index {
            index if index == from => to,
            index if from < index && to >= index => index - 1,
            index if from > index && to <= index => index + 1,
            index => index,
        };
        true
    }

    pub fn clear_upcoming(&mut self) {
        self.tracks.truncate(self.index + 1);
        self.unshuffled = None;
    }

    pub fn is_shuffled(&self) -> bool {
        self.unshuffled.is_some()
    }

    pub fn set_shuffle(&mut self, on: bool, seed: u64) {
        if on == self.is_shuffled() || self.tracks.is_empty() {
            return;
        }
        let current = self.current().map(|track| track.id.clone());
        if on {
            self.unshuffled = Some(self.tracks.clone());
            let first = self.tracks.remove(self.index);
            shuffle(&mut self.tracks, seed | 1);
            self.tracks.insert(0, first);
            self.index = 0;
        } else if let Some(original) = self.unshuffled.take() {
            self.tracks = original;
            self.index = current.and_then(|id| self.tracks.iter().position(|track| track.id == id)).unwrap_or(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tactus_api::fake::tracks;

    fn queue(ids: &[&str], start: usize) -> Queue {
        let mut queue = Queue::default();
        queue.replace(tracks(ids), start);
        queue
    }

    fn ids(queue: &Queue) -> Vec<String> {
        queue.ids()
    }

    #[test]
    fn follows_repeat_modes() {
        let mut queue = queue(&["a", "b"], 1);
        assert_eq!(queue.following(Repeat::Off, false), None);
        assert_eq!(queue.following(Repeat::All, false), Some(0));
        assert_eq!(queue.following(Repeat::One, false), Some(1));
        assert_eq!(queue.following(Repeat::One, true), Some(0));
        queue.select(0);
        assert_eq!(queue.following(Repeat::Off, true), Some(1));
        assert_eq!(queue.preceding(Repeat::All), Some(1));
        assert_eq!(queue.preceding(Repeat::Off), Some(0));
        assert_eq!(Repeat::Off.cycle().cycle().cycle(), Repeat::Off);
    }

    #[test]
    fn inserts_and_appends() {
        let mut queue = queue(&["a", "b", "c"], 0);
        queue.insert_next(tracks(&["x", "y"]));
        queue.append(tracks(&["z"]));
        assert_eq!(ids(&queue), ["a", "x", "y", "b", "c", "z"]);
        assert_eq!(queue.remaining(), 5);
    }

    #[test]
    fn removing_keeps_the_current_track() {
        let mut queue = queue(&["a", "b", "c"], 1);
        queue.remove(0);
        assert_eq!(queue.current().unwrap().id, "b");
        queue.remove(1);
        assert_eq!(queue.current().unwrap().id, "b");
        queue.remove(0);
        assert!(queue.is_empty());
        assert_eq!(queue.remove(5), None);
    }

    #[test]
    fn moving_keeps_the_current_track() {
        let mut queue = queue(&["a", "b", "c", "d"], 1);
        assert!(queue.move_item(0, 3));
        assert_eq!(ids(&queue), ["b", "c", "d", "a"]);
        assert_eq!(queue.current().unwrap().id, "b");
        assert!(queue.move_item(3, 0));
        assert_eq!(queue.current().unwrap().id, "b");
        assert!(queue.move_item(1, 2));
        assert_eq!(queue.current().unwrap().id, "b");
        assert!(!queue.move_item(1, 1));
    }

    #[test]
    fn shuffle_keeps_current_first_and_restores_order() {
        let names = ["a", "b", "c", "d", "e", "f"];
        let mut queue = queue(&names, 2);
        queue.set_shuffle(true, 42);
        assert_eq!(queue.current().unwrap().id, "c");
        assert_eq!(queue.index(), 0);
        let mut sorted = ids(&queue);
        sorted.sort();
        assert_eq!(sorted, names);
        queue.append(tracks(&["g"]));
        queue.set_shuffle(false, 0);
        assert_eq!(ids(&queue), ["a", "b", "c", "d", "e", "f", "g"]);
        assert_eq!(queue.current().unwrap().id, "c");
    }

    #[test]
    fn clears_upcoming_tracks() {
        let mut queue = queue(&["a", "b", "c"], 1);
        queue.clear_upcoming();
        assert_eq!(ids(&queue), ["a", "b"]);
    }

    #[derive(Clone, Debug)]
    enum Edit {
        Select(usize),
        InsertNext(usize),
        Append(usize),
        Remove(usize),
        Move(usize, usize),
        ClearUpcoming,
        Shuffle(bool, u64),
    }

    fn edit() -> impl Strategy<Value = Edit> {
        prop_oneof![
            (0usize..25).prop_map(Edit::Select),
            (0usize..4).prop_map(Edit::InsertNext),
            (0usize..4).prop_map(Edit::Append),
            (0usize..25).prop_map(Edit::Remove),
            (0usize..25, 0usize..25).prop_map(|(from, to)| Edit::Move(from, to)),
            Just(Edit::ClearUpcoming),
            (any::<bool>(), any::<u64>()).prop_map(|(on, seed)| Edit::Shuffle(on, seed)),
        ]
    }

    fn fresh(next: &mut usize, count: usize) -> Vec<Track> {
        (0..count)
            .map(|_| {
                *next += 1;
                tactus_api::fake::track(&next.to_string())
            })
            .collect()
    }

    proptest! {
        #[test]
        fn edits_keep_the_index_valid_and_the_current_track(
            count in 0usize..20,
            start in 0usize..25,
            edits in proptest::collection::vec(edit(), 0..40),
        ) {
            let mut next = 0;
            let mut queue = Queue::default();
            queue.replace(fresh(&mut next, count), start);
            for edit in edits {
                let (current, index, len) = (queue.current().map(|track| track.id.clone()), queue.index(), queue.len());
                let keeps_current = match edit {
                    Edit::Select(row) => {
                        queue.select(row);
                        row >= len
                    }
                    Edit::InsertNext(added) => {
                        queue.insert_next(fresh(&mut next, added));
                        prop_assert_eq!(queue.len(), len + added);
                        len > 0
                    }
                    Edit::Append(added) => {
                        queue.append(fresh(&mut next, added));
                        prop_assert_eq!(queue.len(), len + added);
                        len > 0
                    }
                    Edit::Remove(row) => {
                        let removed = queue.remove(row).is_some();
                        prop_assert_eq!(queue.len(), len - usize::from(removed));
                        row != index
                    }
                    Edit::Move(from, to) => {
                        queue.move_item(from, to);
                        true
                    }
                    Edit::ClearUpcoming => {
                        queue.clear_upcoming();
                        prop_assert_eq!(queue.remaining(), 0);
                        true
                    }
                    Edit::Shuffle(on, seed) => {
                        queue.set_shuffle(on, seed);
                        prop_assert_eq!(queue.len(), len);
                        true
                    }
                };
                prop_assert!(queue.index() < queue.len().max(1));
                if keeps_current {
                    prop_assert_eq!(queue.current().map(|track| track.id.clone()), current);
                }
            }
        }

        #[test]
        fn shuffling_round_trips(count in 1usize..40, start in 0usize..40, seed: u64) {
            let mut next = 0;
            let mut queue = Queue::default();
            queue.replace(fresh(&mut next, count), start);
            let (original, current) = (queue.ids(), queue.current().map(|track| track.id.clone()));
            queue.set_shuffle(true, seed);
            let mut shuffled = queue.ids();
            prop_assert_eq!(queue.index(), 0);
            prop_assert_eq!(queue.current().map(|track| track.id.clone()), current.clone());
            shuffled.sort();
            let mut sorted = original.clone();
            sorted.sort();
            prop_assert_eq!(shuffled, sorted);
            queue.set_shuffle(false, seed);
            prop_assert_eq!(queue.ids(), original);
            prop_assert_eq!(queue.current().map(|track| track.id.clone()), current);
        }
    }

    proptest! {
        #[test]
        fn reinserting_removed_tracks_restores_the_queue(
            count in 1usize..30,
            start in 0usize..30,
            picks in proptest::collection::btree_set(0usize..30, 0..10),
        ) {
            let mut next = 0;
            let mut queue = Queue::default();
            queue.replace(fresh(&mut next, count), start);
            let (original, current) = (queue.ids(), queue.current().map(|track| track.id.clone()));
            let removed: Vec<(usize, Track)> =
                picks.iter().rev().filter_map(|&index| Some((index, queue.remove(index)?))).collect();
            for (index, track) in removed.into_iter().rev() {
                queue.insert_at(index, track);
            }
            prop_assert_eq!(queue.ids(), original);
            if !picks.contains(&start.min(count - 1)) {
                prop_assert_eq!(queue.current().map(|track| track.id.clone()), current);
            }
        }
    }
}
