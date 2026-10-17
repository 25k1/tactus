use super::*;

pub fn now_playing(player: &Player) -> Option<String> {
    let track = player.track.as_ref()?;
    Some(tactus_core::announce::format_track(&player.settings.track_format, track))
}

pub fn position(player: &Player) -> String {
    let duration = player.track.as_ref().map_or(0, |track| track.duration_ms);
    format!("{} / {}", format_time(player.position_ms), format_time(duration))
}

pub fn now_playing_details(player: &Player) -> String {
    let Some(track) = &player.track else {
        return fl!("now-playing-nothing");
    };
    let mut lines = vec![track.full_title(), track.artist.clone()];
    if !track.album.is_empty() {
        lines.push(track.album.clone());
    }
    lines.push(position(player));
    lines.push(if player.playing { fl!("state-playing") } else { fl!("state-paused") });
    if let Some(remaining) = player.sleep_remaining() {
        lines.push(fl!("sleep-remaining", time = format_time(remaining.as_millis() as u64)));
    } else if let Some(timer) = player.sleep_timer() {
        lines.push(match timer {
            SleepTimer::EndOfQueue => fl!("notice-sleep-end-of-queue"),
            _ => fl!("notice-sleep-end-of-track"),
        });
    }
    lines.join("\n")
}

pub fn track_info(info: &TrackInfo) -> String {
    let track = &info.track;
    let yes_no = |value: bool| if value { fl!("info-yes") } else { fl!("info-no") };
    let lyrics = match (info.synced_lyrics, info.lyrics) {
        (true, _) => fl!("info-lyrics-synced"),
        (false, true) => fl!("info-lyrics-text"),
        _ => fl!("info-no"),
    };
    let fields = [
        (fl!("info-title"), track.title.clone()),
        (fl!("info-version"), track.version.clone()),
        (fl!("info-artists"), info.artists.join(", ")),
        (fl!("info-album"), track.album.clone()),
        (fl!("info-album-type"), info.album_type.clone()),
        (fl!("info-year"), if track.year > 0 { track.year.to_string() } else { String::new() }),
        (fl!("info-release-date"), info.release_date.clone()),
        (fl!("info-genre"), track.genre.clone()),
        (fl!("info-duration"), format_time(track.duration_ms)),
        (fl!("info-labels"), info.labels.join(", ")),
        (fl!("info-explicit"), yes_no(track.explicit)),
        (fl!("info-lyrics"), lyrics),
        (fl!("info-link"), track.link()),
    ];
    fields
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn artist_facts(artist: &Artist) -> String {
    let facts = [
        (artist.listeners > 0).then(|| fl!("artist-listeners", count = artist.listeners)),
        (artist.likes > 0).then(|| fl!("artist-likes", count = artist.likes)),
        (!artist.countries.is_empty()).then(|| artist.countries.clone()),
        (artist.since > 0).then(|| fl!("artist-since", year = artist.since)),
    ];
    facts.into_iter().flatten().collect::<Vec<_>>().join(". ")
}

pub fn describe(page: &Page) -> String {
    let facts = match &page.subject {
        Some(Item::Artist(artist)) => artist_facts(artist),
        _ => String::new(),
    };
    windows_lines(&join_present([facts, page.description.trim().to_owned()], "\n\n"))
}
