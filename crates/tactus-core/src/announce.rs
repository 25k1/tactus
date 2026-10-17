use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tactus_api::Track;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Announcement {
    Track,
    Playback,
    Volume,
    Seek,
    Modes,
    Likes,
    Queue,
    Playlists,
    Downloads,
    Search,
    Lyrics,
    Errors,
    Voice,
}

impl Announcement {
    pub const ALL: [Announcement; 13] = [
        Announcement::Track,
        Announcement::Playback,
        Announcement::Volume,
        Announcement::Seek,
        Announcement::Modes,
        Announcement::Likes,
        Announcement::Queue,
        Announcement::Playlists,
        Announcement::Downloads,
        Announcement::Search,
        Announcement::Lyrics,
        Announcement::Errors,
        Announcement::Voice,
    ];

    pub fn defaults() -> BTreeSet<Announcement> {
        Self::ALL.into_iter().filter(|kind| *kind != Announcement::Lyrics).collect()
    }
}

pub const TRACK_FORMATS: [&str; 5] = [
    "{title} — {artist}",
    "{artist} — {title}",
    "{title}",
    "{title} — {artist}, {album}",
    "{title}, {artist}, {duration}",
];

pub fn format_time(ms: u64) -> String {
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

pub fn format_track(template: &str, track: &Track) -> String {
    const SEPARATORS: &[char] = &[' ', ',', '—', '–', '-', '|', ':', '·', '/'];
    let year = if track.year > 0 { track.year.to_string() } else { String::new() };
    let fields = [
        ("{title}", track.full_title()),
        ("{artist}", track.artist.clone()),
        ("{album}", track.album.clone()),
        ("{year}", year),
        ("{genre}", track.genre.clone()),
        ("{duration}", if track.duration_ms > 0 { format_time(track.duration_ms) } else { String::new() }),
    ];
    let mut text = template.to_owned();
    for (placeholder, value) in &fields {
        text = text.replace(placeholder, value);
    }
    let mut cleaned = String::with_capacity(text.len());
    for part in text.split(", ").filter(|part| !part.trim_matches(SEPARATORS).is_empty()) {
        if !cleaned.is_empty() {
            cleaned.push_str(", ");
        }
        cleaned.push_str(part);
    }
    match cleaned.trim_matches(SEPARATORS) {
        "" => track.full_title(),
        text => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tactus_api::fake::track;

    #[test]
    fn formats_time() {
        assert_eq!(format_time(0), "0:00");
        assert_eq!(format_time(61_500), "1:01");
        assert_eq!(format_time(3_725_000), "1:02:05");
    }

    #[test]
    fn formats_tracks_and_drops_empty_fields() {
        let mut track = track("1");
        assert_eq!(format_track("{title} — {artist}", &track), "Track 1 — Artist");
        track.album.clear();
        assert_eq!(format_track("{title} — {artist}, {album}", &track), "Track 1 — Artist");
        assert_eq!(format_track("{title}, {artist}, {duration}", &track), "Track 1, Artist, 3:00");
        track.version = "Live".into();
        assert_eq!(format_track("{title}", &track), "Track 1 (Live)");
        assert_eq!(format_track("{album}", &track), "Track 1 (Live)");
    }

    #[test]
    fn lyrics_are_off_by_default() {
        let defaults = Announcement::defaults();
        assert!(!defaults.contains(&Announcement::Lyrics));
        assert!(defaults.contains(&Announcement::Track));
    }
}
