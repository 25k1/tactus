use std::fmt::Write;
use tactus_api::{Destination, Track, View};

pub const PLAY_FROM: &str = "desktop_win-own_tracks-track-default";
pub const MY_WAVE: &str = "user:onyourwave";

pub fn uri(track: &Track) -> String {
    match track.album_id.as_str() {
        "" => format!("yandexmusic://track/{}", track.id),
        album => format!("yandexmusic://album/{album}/track/{}", track.id),
    }
}

pub fn id(location: &str) -> Option<String> {
    match tactus_yandex::destination(location)? {
        Destination::View(View::Track(id)) if !id.is_empty() => Some(id),
        _ => None,
    }
}

pub fn web_link(track: &Track) -> String {
    use tactus_api::Provider;
    tactus_yandex::YandexProvider.share_link(&tactus_api::Item::Track(track.clone())).unwrap_or_default()
}

pub fn display(track: &Track) -> String {
    match track.artist.as_str() {
        "" => track.full_title(),
        artist => format!("{artist} - {}", track.full_title()),
    }
}

pub fn duration(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

pub fn length_ms(track: &Track) -> i32 {
    match track.duration_ms {
        0 => -1,
        milliseconds => milliseconds.min(i32::MAX as u64) as i32,
    }
}

pub fn playlist(tracks: &[Track]) -> String {
    tracks.iter().fold(String::from("#EXTM3U\n"), |mut text, track| {
        let _ = writeln!(text, "{}", uri(track));
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, album: &str) -> Track {
        Track {
            id: id.into(),
            album_id: album.into(),
            title: "Блюз".into(),
            artist: "Кино".into(),
            duration_ms: 185_000,
            ..Track::default()
        }
    }

    #[test]
    fn locations_round_trip_to_track_ids() {
        assert_eq!(uri(&track("42", "7")), "yandexmusic://album/7/track/42");
        assert_eq!(id(&uri(&track("42", "7"))).as_deref(), Some("42"));
        assert_eq!(id(&uri(&track("42", ""))).as_deref(), Some("42"));
        assert_eq!(id("https://music.yandex.ru/album/7/track/42?utm=1").as_deref(), Some("42"));
        assert_eq!(id("https://music.yandex.ru/album/7"), None);
        assert_eq!(id("C:\\Music\\song.mp3"), None);
    }

    #[test]
    fn writes_playlists_without_titles() {
        let text = playlist(&[track("42", "7"), Track { id: "5".into(), title: "Intro".into(), ..Track::default() }]);
        assert_eq!(text, "#EXTM3U\nyandexmusic://album/7/track/42\nyandexmusic://track/5\n");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(duration(185_000), "3:05");
        assert_eq!(duration(3_723_000), "1:02:03");
        assert_eq!(duration(0), "0:00");
    }

    #[test]
    fn reports_unknown_lengths_to_winamp() {
        assert_eq!(length_ms(&track("1", "")), 185_000);
        assert_eq!(length_ms(&Track::default()), -1);
        assert_eq!(length_ms(&Track { duration_ms: u64::MAX, ..Track::default() }), i32::MAX);
    }
}
