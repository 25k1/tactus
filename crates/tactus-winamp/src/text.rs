use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};
use tactus_api::error::{Error, ErrorKind};
use tactus_api::{AlbumKind, Item, Section, SectionKind};

const SOURCES: [(&str, &str); 3] = [
    ("en", include_str!("../../../i18n/en/tactus.ftl")),
    ("ru", include_str!("../../../i18n/ru/tactus.ftl")),
    ("ka", include_str!("../../../i18n/ka/tactus.ftl")),
];

static LANGUAGE: OnceLock<&'static str> = OnceLock::new();
static MESSAGES: LazyLock<HashMap<&'static str, HashMap<&'static str, &'static str>>> =
    LazyLock::new(|| SOURCES.iter().map(|(code, source)| (*code, parse(source))).collect());

fn parse(source: &'static str) -> HashMap<&'static str, &'static str> {
    source
        .lines()
        .filter(|line| !line.starts_with([' ', '\t', '#', '.']))
        .filter_map(|line| line.split_once(" = "))
        .filter(|(_, value)| !value.trim().is_empty() && !value.contains("->"))
        .map(|(key, value)| (key.trim(), value))
        .collect()
}

pub fn system_language() -> &'static str {
    let language = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match language {
        0x19 | 0x23 | 0x3f => "ru",
        0x37 => "ka",
        _ => "en",
    }
}

pub fn set_language(code: &'static str) {
    let _ = LANGUAGE.set(code);
}

fn lookup_in(language: &str, key: &str) -> Option<&'static str> {
    MESSAGES.get(language).and_then(|messages| messages.get(key)).or_else(|| MESSAGES["en"].get(key)).copied()
}

fn lookup(key: &str) -> Option<&'static str> {
    lookup_in(LANGUAGE.get().copied().unwrap_or("en"), key)
}

pub fn language_name(code: &str) -> String {
    lookup_in(code, "language-name").unwrap_or(code).to_owned()
}

pub fn text(key: &str) -> String {
    lookup(key).map_or_else(|| key.to_owned(), str::to_owned)
}

pub fn text_with(key: &str, arguments: &[(&str, &str)]) -> String {
    arguments.iter().fold(text(key), |message, (name, value)| message.replace(&format!("{{ ${name} }}"), value))
}

pub fn plain(text: &str) -> String {
    text.split('\t').next().unwrap_or_default().replace('&', "")
}

pub fn error(error: &Error) -> String {
    let detail = error.to_string();
    match error.kind() {
        ErrorKind::Network => text("error-network"),
        ErrorKind::SignedOut => text("error-signed-out"),
        ErrorKind::Expired => text("error-expired"),
        ErrorKind::NotFound => text("error-not-found"),
        ErrorKind::Subscription => text("error-subscription"),
        ErrorKind::Region => text("error-region"),
        ErrorKind::RateLimited => text("error-rate-limited"),
        ErrorKind::Server => text("error-server"),
        ErrorKind::Unsupported => text("error-unsupported"),
        ErrorKind::Audio | ErrorKind::Storage | ErrorKind::Other => text_with("error-other", &[("detail", &detail)]),
    }
}

fn section_key(kind: SectionKind) -> Option<&'static str> {
    Some(match kind {
        SectionKind::Best => "section-all",
        SectionKind::Tracks => "section-tracks",
        SectionKind::Albums => "section-albums",
        SectionKind::AppearsOn => "section-appears-on",
        SectionKind::Artists => "section-artists",
        SectionKind::SimilarArtists => "section-similar-artists",
        SectionKind::Playlists => "section-playlists",
        SectionKind::Podcasts => "section-podcasts",
        SectionKind::Episodes => "section-episodes",
        SectionKind::Suggestions => "section-suggestions",
        SectionKind::DislikedTracks => "section-disliked-tracks",
        SectionKind::DislikedArtists => "section-disliked-artists",
        SectionKind::Similar => "section-similar",
        SectionKind::Versions => "section-versions",
        SectionKind::Editions => "section-editions",
        SectionKind::Links => "section-links",
        SectionKind::Charts => "section-charts",
        SectionKind::FollowedReleases => "section-followed-releases",
        SectionKind::GenreReleases => "section-genre-releases",
        SectionKind::RecommendedArtists => "section-recommended-artists",
        SectionKind::Genres => "section-genres",
        SectionKind::Categories => "section-categories",
        SectionKind::Pinned => "section-pinned",
        SectionKind::Moments => "section-moments",
        SectionKind::Discography => "section-discography",
        SectionKind::Upcoming => "section-upcoming",
        SectionKind::Presaves => "section-presaves",
        SectionKind::Concerts => "section-concerts",
        SectionKind::Generated => "section-generated",
        SectionKind::NewReleases => "section-new-releases",
        SectionKind::NewPlaylists => "section-new-playlists",
        SectionKind::NewEpisodes => "section-new-episodes",
        SectionKind::Stations | SectionKind::Other => return None,
    })
}

fn station_category(code: &str) -> String {
    match code {
        "genre" => text("station-genres"),
        "mood" => text("station-moods"),
        "activity" => text("station-activities"),
        "epoch" => text("station-eras"),
        "local" => text("station-local"),
        code => code.to_owned(),
    }
}

pub fn section(section: &Section) -> String {
    let title = section.title.as_str();
    match (section.kind, section_key(section.kind)) {
        (SectionKind::Stations, _) => station_category(title),
        (SectionKind::NewReleases | SectionKind::NewPlaylists | SectionKind::Other, _) if !title.is_empty() => {
            title.to_owned()
        }
        (SectionKind::NewEpisodes, Some(key)) if !title.is_empty() => format!("{}: {title}", text(key)),
        (_, Some(key)) => text(key),
        (_, None) => title.to_owned(),
    }
}

pub fn chart(region: &str) -> String {
    match lookup(&format!("chart-{region}")) {
        Some(label) => label.to_owned(),
        None => region.to_owned(),
    }
}

fn album_kind(kind: AlbumKind) -> String {
    text(match kind {
        AlbumKind::Music => "kind-album",
        AlbumKind::Single => "kind-single",
        AlbumKind::Podcast => "kind-podcast",
        AlbumKind::Audiobook => "kind-audiobook",
    })
}

fn joined(parts: &[&str]) -> String {
    parts.iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(", ")
}

fn count(count: u32) -> String {
    if count == 0 { String::new() } else { text_with("winamp-track-count", &[("count", &count.to_string())]) }
}

fn year(year: u32) -> String {
    if year == 0 { String::new() } else { year.to_string() }
}

pub fn title(item: &Item) -> String {
    match item {
        Item::Chart(chart_item) => chart(&chart_item.region),
        item => item.title(),
    }
}

pub fn details(item: &Item) -> String {
    match item {
        Item::Track(track) => joined(&[&track.artist, &track.album]),
        Item::Album(album) => {
            joined(&[&album_kind(album.kind), &album.artist, &year(album.year), &count(album.track_count)])
        }
        Item::Artist(artist) => joined(&[&text("kind-artist"), &artist.genres]),
        Item::Playlist(playlist) => joined(&[&text("kind-playlist"), &playlist.owner, &count(playlist.track_count)]),
        Item::Station(station) => joined(&[&text("kind-station"), &station_category(&station.category)]),
        Item::Concert(concert) => joined(&[&concert.city, &concert.place, &concert.date]),
        Item::Link(link) => link.url.clone(),
        Item::Tag(_) | Item::Genre(_) | Item::Category(_) | Item::Chart(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_simple_messages_and_skips_plurals() {
        let messages = parse(
            "a = One\nplural = { $count ->\n    [one] x\n   *[other] y\n}\nb = Two { $name }\nc = { $a } of { $b }\n",
        );
        assert_eq!(messages.get("a"), Some(&"One"));
        assert_eq!(messages.get("b"), Some(&"Two { $name }"));
        assert_eq!(messages.get("c"), Some(&"{ $a } of { $b }"));
        assert!(!messages.contains_key("plural"));
    }

    #[test]
    fn every_plugin_message_exists_in_every_language() {
        for (code, source) in SOURCES {
            let messages = parse(source);
            let keys = [
                "winamp-menu",
                "winamp-enqueue",
                "winamp-track-count",
                "winamp-settings",
                "winamp-language",
                "winamp-language-automatic",
                "winamp-account",
                "winamp-hotkeys",
                "winamp-hotkeys-help",
                "winamp-open-in-browser",
                "winamp-general-missing",
                "winamp-browser-signed-out",
                "winamp-settings-button",
                "winamp-what-playing",
                "winamp-playing",
                "winamp-paused",
                "winamp-stopped",
                "winamp-time-of",
                "winamp-playlist-empty",
                "action-track-info",
                "menu-copy-link",
                "notice-link-copied",
                "dialog-ok",
                "dialog-cancel",
                "dialog-close",
                "language-name",
            ];
            for key in keys {
                assert!(messages.contains_key(key), "{code} is missing {key}");
            }
        }
    }

    #[test]
    fn fills_arguments_and_strips_mnemonics() {
        assert_eq!(text_with("status-signed-in", &[("name", "beka")]), "Signed in as beka");
        assert_eq!(plain("Remove from &queue\tDelete"), "Remove from queue");
        assert_eq!(text("no-such-key"), "no-such-key");
        assert_eq!(language_name("ru"), "Русский");
    }
}
