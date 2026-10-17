use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SectionKind {
    Best,
    Tracks,
    Albums,
    AppearsOn,
    Artists,
    SimilarArtists,
    Playlists,
    Podcasts,
    Episodes,
    Suggestions,
    DislikedTracks,
    DislikedArtists,
    Stations,
    Similar,
    Versions,
    Editions,
    Links,
    Charts,
    FollowedReleases,
    GenreReleases,
    RecommendedArtists,
    Genres,
    NewEpisodes,
    Pinned,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub kind: SectionKind,
    pub title: String,
    pub items: Vec<Item>,
    pub more: Option<u32>,
}

impl Section {
    pub fn new(kind: SectionKind, title: impl Into<String>, items: Vec<Item>) -> Self {
        Self { kind, title: title.into(), items, more: None }
    }

    pub fn with_more(self, more: Option<u32>) -> Self {
        Self { more, ..self }
    }

    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.items.iter().filter_map(Item::track)
    }

    pub fn extend(&mut self, paged: Paged) -> usize {
        let before = self.items.len();
        for item in paged.items {
            if !self.items.contains(&item) {
                self.items.push(item);
            }
        }
        self.more = paged.next;
        self.items.len() - before
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paged {
    pub items: Vec<Item>,
    pub next: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub title: String,
    pub description: String,
    pub subject: Option<Item>,
    pub sections: Vec<Section>,
}

impl Page {
    pub fn tracks(&self) -> Vec<Track> {
        self.sections
            .iter()
            .filter(|section| section.kind != SectionKind::Suggestions)
            .flat_map(Section::tracks)
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Search(String),
    Waves,
    ForYou,
    Collection,
    Playlists,
    Radio,
    Podcasts,
    History,
    Album(String),
    Artist(String),
    Playlist(PlaylistKey),
    Track(String),
    Tag(String),
    Genre(String),
    User(String),
    Chart(String),
}

impl View {
    pub fn of(item: &Item) -> Option<View> {
        Some(match item {
            Item::Album(album) => View::Album(album.id.clone()),
            Item::Artist(artist) => View::Artist(artist.id.clone()),
            Item::Playlist(playlist) => View::Playlist(playlist.key),
            Item::Tag(tag) => View::Tag(tag.id.clone()),
            Item::Genre(genre) => View::Genre(genre.id.clone()),
            Item::Chart(chart) => View::Chart(chart.region.clone()),
            Item::Track(_) | Item::Station(_) | Item::Link(_) => return None,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Suggestions {
    pub best: Option<Item>,
    pub texts: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::track;
    use proptest::collection::vec;
    use proptest::prelude::*;

    fn item(id: &u8) -> Item {
        Item::Track(track(&id.to_string()))
    }

    proptest! {
        #[test]
        fn extending_adds_only_new_items(first in vec(0u8..20, 0..30), second in vec(0u8..20, 0..30), next: Option<u32>) {
            let mut unique = first;
            unique.sort_unstable();
            unique.dedup();
            let mut section = Section::new(SectionKind::Tracks, "", unique.iter().map(item).collect()).with_more(Some(1));
            let before = section.items.len();
            let added = section.extend(Paged { items: second.iter().map(item).collect(), next });
            prop_assert_eq!(section.items.len(), before + added);
            prop_assert!(second.iter().all(|id| section.items.contains(&item(id))));
            prop_assert!(section.items.iter().enumerate().all(|(index, item)| !section.items[..index].contains(item)));
            prop_assert_eq!(section.more, next);
        }
    }
}
