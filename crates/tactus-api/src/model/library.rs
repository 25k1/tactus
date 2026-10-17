use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Like,
    Dislike,
}

impl Relation {
    pub fn opposite(self) -> Self {
        match self {
            Relation::Like => Relation::Dislike,
            Relation::Dislike => Relation::Like,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Track(Track),
    Album(String),
    Artist(String),
    Playlist(PlaylistKey),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    pub liked_tracks: HashSet<String>,
    pub disliked_tracks: HashSet<String>,
    pub liked_albums: HashSet<String>,
    pub liked_artists: HashSet<String>,
    pub disliked_artists: HashSet<String>,
    pub liked_playlists: HashSet<PlaylistKey>,
    pub pinned: Vec<Item>,
}

impl Library {
    pub fn is_pinned(&self, target: &Target) -> bool {
        self.pinned.iter().any(|item| item.target().as_ref() == Some(target))
    }

    pub fn set_pinned(&mut self, item: &Item, on: bool) {
        let target = item.target();
        self.pinned.retain(|pinned| pinned.target() != target);
        if on {
            self.pinned.insert(0, item.clone());
        }
    }

    fn set(&mut self, target: &Target, relation: Relation) -> Option<&mut HashSet<String>> {
        Some(match (target, relation) {
            (Target::Track(_), Relation::Like) => &mut self.liked_tracks,
            (Target::Track(_), Relation::Dislike) => &mut self.disliked_tracks,
            (Target::Album(_), Relation::Like) => &mut self.liked_albums,
            (Target::Artist(_), Relation::Like) => &mut self.liked_artists,
            (Target::Artist(_), Relation::Dislike) => &mut self.disliked_artists,
            _ => return None,
        })
    }

    fn id(target: &Target) -> &str {
        match target {
            Target::Track(track) => &track.id,
            Target::Album(id) | Target::Artist(id) => id,
            Target::Playlist(_) => "",
        }
    }

    pub fn has(&self, target: &Target, relation: Relation) -> bool {
        match (target, relation) {
            (Target::Playlist(key), Relation::Like) => self.liked_playlists.contains(key),
            (Target::Playlist(_), Relation::Dislike) => false,
            (Target::Track(track), Relation::Like) => self.liked_tracks.contains(&track.id),
            (Target::Track(track), Relation::Dislike) => self.disliked_tracks.contains(&track.id),
            (Target::Album(id), Relation::Like) => self.liked_albums.contains(id),
            (Target::Album(_), Relation::Dislike) => false,
            (Target::Artist(id), Relation::Like) => self.liked_artists.contains(id),
            (Target::Artist(id), Relation::Dislike) => self.disliked_artists.contains(id),
        }
    }

    pub fn apply(&mut self, target: &Target, relation: Relation, on: bool) {
        if let Target::Playlist(key) = target {
            if relation == Relation::Like {
                if on {
                    self.liked_playlists.insert(*key);
                } else {
                    self.liked_playlists.remove(key);
                }
            }
            return;
        }
        let id = Self::id(target).to_owned();
        if on && let Some(set) = self.set(target, relation.opposite()) {
            set.remove(&id);
        }
        if let Some(set) = self.set(target, relation) {
            if on {
                set.insert(id);
            } else {
                set.remove(&id);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaylistEdit {
    Rename(String),
    Visibility(bool),
    Delete,
    Description(String),
    Insert { at: u32, tracks: Vec<Track> },
    Remove(Vec<(usize, Track)>),
    Restore(Vec<(usize, Track)>),
    Move { from: usize, to: usize, track: Track },
}
