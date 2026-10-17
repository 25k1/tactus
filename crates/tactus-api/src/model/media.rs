use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistKey {
    pub owner: u64,
    pub kind: u32,
}

impl PlaylistKey {
    pub fn seed(self) -> String {
        format!("playlist:{}_{}", self.owner, self.kind)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub version: String,
    pub artist: String,
    pub artist_id: String,
    pub album: String,
    pub album_id: String,
    pub year: u32,
    pub genre: String,
    pub cover: String,
    pub duration_ms: u64,
    pub explicit: bool,
    pub episode: bool,
    pub loudness: Option<Loudness>,
    #[serde(skip)]
    pub played: bool,
    #[serde(skip)]
    pub downloaded: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Loudness {
    pub integrated: f32,
    pub peak: f32,
}

impl Track {
    pub fn full_title(&self) -> String {
        if self.version.is_empty() { self.title.clone() } else { format!("{} ({})", self.title, self.version) }
    }

    pub fn link(&self) -> String {
        if self.album_id.is_empty() {
            format!("https://music.yandex.ru/track/{}", self.id)
        } else {
            format!("https://music.yandex.ru/album/{}/track/{}", self.album_id, self.id)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlbumKind {
    #[default]
    Music,
    Single,
    Podcast,
    Audiobook,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_id: String,
    pub year: u32,
    pub track_count: u32,
    pub genre: String,
    pub kind: AlbumKind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub genres: String,
    pub listeners: u64,
    pub likes: u64,
    pub countries: String,
    pub since: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playlist {
    pub key: PlaylistKey,
    pub title: String,
    pub owner: String,
    pub track_count: u32,
    pub public: bool,
    pub description: String,
    pub collective: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Station {
    pub seed: String,
    pub name: String,
    pub category: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tag {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Genre {
    pub id: String,
    pub title: String,
    pub seed: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Link {
    pub title: String,
    pub url: String,
}

pub const CHART_REGIONS: [&str; 6] = ["russia", "world", "kazakhstan", "belarus", "uzbekistan", "georgia"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chart {
    pub region: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Track(Track),
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
    Station(Station),
    Tag(Tag),
    Genre(Genre),
    Link(Link),
    Chart(Chart),
}

impl Item {
    pub fn track(&self) -> Option<&Track> {
        match self {
            Item::Track(track) => Some(track),
            _ => None,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Item::Track(track) => track.full_title(),
            Item::Album(album) => album.title.clone(),
            Item::Artist(artist) => artist.name.clone(),
            Item::Playlist(playlist) => playlist.title.clone(),
            Item::Station(station) => station.name.clone(),
            Item::Tag(tag) => tag.title.clone(),
            Item::Genre(genre) => genre.title.clone(),
            Item::Link(link) => link.title.clone(),
            Item::Chart(chart) => chart.region.clone(),
        }
    }

    pub fn target(&self) -> Option<Target> {
        Some(match self {
            Item::Track(track) => Target::Track(track.clone()),
            Item::Album(album) => Target::Album(album.id.clone()),
            Item::Artist(artist) => Target::Artist(artist.id.clone()),
            Item::Playlist(playlist) => Target::Playlist(playlist.key),
            Item::Station(_) | Item::Tag(_) | Item::Genre(_) | Item::Link(_) | Item::Chart(_) => return None,
        })
    }

    pub fn link(&self) -> Option<String> {
        Some(match self {
            Item::Track(track) => track.link(),
            Item::Album(album) => format!("https://music.yandex.ru/album/{}", album.id),
            Item::Artist(artist) => format!("https://music.yandex.ru/artist/{}", artist.id),
            Item::Playlist(playlist) => {
                format!("https://music.yandex.ru/users/{}/playlists/{}", playlist.key.owner, playlist.key.kind)
            }
            Item::Tag(tag) => format!("https://music.yandex.ru/tag/{}", tag.id),
            Item::Link(link) => link.url.clone(),
            Item::Chart(_) => "https://music.yandex.ru/chart".to_owned(),
            Item::Station(_) | Item::Genre(_) => return None,
        })
    }

    pub fn seed(&self) -> Option<String> {
        Some(match self {
            Item::Track(track) => format!("track:{}", track.id),
            Item::Album(album) => format!("album:{}", album.id),
            Item::Artist(artist) => format!("artist:{}", artist.id),
            Item::Playlist(playlist) => playlist.key.seed(),
            Item::Station(station) => station.seed.clone(),
            Item::Genre(genre) if !genre.seed.is_empty() => genre.seed.clone(),
            Item::Genre(_) => return None,
            Item::Tag(_) | Item::Link(_) | Item::Chart(_) => return None,
        })
    }
}
