use super::*;

fn chart_position(value: &Value) -> Option<ChartPosition> {
    let position = u32::try_from(value.num("position")).ok().filter(|position| *position > 0)?;
    let shift = value.at("shift").as_i64().unwrap_or_default();
    let trend = match (value.text("progress"), shift) {
        ("new", _) => Trend::New,
        (_, shift) if shift > 0 => Trend::Up(shift as u32),
        (_, shift) if shift < 0 => Trend::Down(shift.unsigned_abs() as u32),
        _ => Trend::Same,
    };
    Some(ChartPosition { position, trend })
}

pub fn track(value: &Value) -> Option<Track> {
    let chart = chart_position(value.at("chart"));
    let value = value.unwrap("track").unwrap("fullModel");
    let id = non_empty(value.id("id"))?;
    let title = value.text("title");
    if title.is_empty() {
        return None;
    }
    let album = value.at("/albums/0");
    let artists = value.list("artists");
    Some(Track {
        id,
        title: title.to_owned(),
        version: value.text("version").to_owned(),
        artist: names(artists),
        artist_id: artists.first().map(|artist| artist.id("id")).unwrap_or_default(),
        album: album.text("title").to_owned(),
        album_id: album.id("id"),
        year: album.num("year") as u32,
        genre: album.text("genre").to_owned(),
        cover: cover(value.text("coverUri")),
        duration_ms: value.num("durationMs"),
        explicit: value.text("contentWarning") == "explicit" || value.flag("explicit"),
        episode: value.text("type") == "podcast-episode" || value.flag("rememberPosition"),
        loudness: loudness(value.at("r128")),
        isrc: value.text("isrc").to_owned(),
        published: value.text("pubDate").get(..10).unwrap_or_default().to_owned(),
        chart,
        played: false,
        downloaded: false,
    })
}

fn loudness(value: &Value) -> Option<Loudness> {
    let level = |key: &str| value.at(key).as_f64().map(|level| level as f32);
    Some(Loudness { integrated: level("i")?, peak: level("tp").unwrap_or(0.0) })
}

pub fn tracks(values: &[Value]) -> Vec<Track> {
    let mut seen = HashSet::new();
    values.iter().filter_map(track).filter(|track| seen.insert(track.id.clone())).collect()
}

pub fn album(value: &Value) -> Option<Album> {
    let value = value.unwrap("album");
    let id = non_empty(value.id("id"))?;
    let artists = value.list("artists");
    Some(Album {
        id,
        title: value.text("title").to_owned(),
        artist: names(artists),
        artist_id: artists.first().map(|artist| artist.id("id")).unwrap_or_default(),
        year: value.num("year") as u32,
        track_count: value.num("trackCount") as u32,
        genre: value.text("genre").to_owned(),
        kind: match value.text("type") {
            "podcast" => AlbumKind::Podcast,
            "audiobook" => AlbumKind::Audiobook,
            "single" => AlbumKind::Single,
            _ => AlbumKind::Music,
        },
        label_id: value.at("/labels/0").id("id"),
        label: value.at("/labels/0").text("name").to_owned(),
        release_date: value.text("releaseDate").get(..10).unwrap_or_default().to_owned(),
        upcoming: chrono::DateTime::parse_from_rfc3339(value.text("releaseDate"))
            .is_ok_and(|date| date > chrono::Utc::now()),
    })
}

pub fn artist(value: &Value) -> Option<Artist> {
    let value = value.unwrap("artist");
    Some(Artist {
        id: non_empty(value.id("id"))?,
        name: value.text("name").to_owned(),
        genres: value.list("genres").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        listeners: 0,
        likes: value.num("likesCount"),
        countries: value.list("countries").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        since: value.text("initDate").get(..4).and_then(|year| year.parse().ok()).unwrap_or_default(),
    })
}

pub fn playlist_key(value: &Value) -> Option<PlaylistKey> {
    let owner = match value.num("uid") {
        0 => value.at("owner").num("uid"),
        uid => uid,
    };
    let kind = value.num("kind") as u32;
    (owner != 0 && kind != 0).then_some(PlaylistKey { owner, kind })
}

pub fn liked_playlist_key(value: &Value) -> Option<PlaylistKey> {
    playlist_key(value.unwrap("playlist"))
}

pub fn playlist(value: &Value) -> Option<Playlist> {
    let value = value.unwrap("playlist");
    let value = if value.get("kind").is_none() { value.unwrap("data") } else { value };
    let owner = value.at("owner");
    Some(Playlist {
        key: playlist_key(value)?,
        title: value.text("title").to_owned(),
        owner: match owner.text("name") {
            "" => owner.text("login").to_owned(),
            name => name.to_owned(),
        },
        track_count: value.num("trackCount") as u32,
        public: value.text("visibility") == "public",
        description: value.text("description").to_owned(),
        collective: value.flag("collective"),
    })
}

pub fn wave_station(value: &Value) -> Option<Item> {
    let value = value.unwrap("wave");
    let seeds: Vec<&str> = match value.at("seeds") {
        Value::String(seeds) => vec![seeds.as_str()],
        seeds => seeds.as_array().map(|seeds| seeds.iter().filter_map(Value::as_str).collect()).unwrap_or_default(),
    };
    let seed = if seeds.is_empty() { value.text("stationId").to_owned() } else { seeds.join(",") };
    let name = [value.text("name"), value.text("title")].into_iter().find(|name| !name.is_empty());
    (!seed.is_empty())
        .then(|| Item::Station(Station { seed, name: name.unwrap_or_default().to_owned(), category: String::new() }))
}

pub fn similar_entities(value: &Value) -> Vec<Item> {
    result(value)
        .list("items")
        .iter()
        .filter_map(|entry| {
            let data = entry.at("data");
            match entry.text("type") {
                "album_item" => album(data).map(Item::Album),
                "artist_item" => artist(data).map(Item::Artist),
                "playlist_item" => playlist(data).map(Item::Playlist),
                "wave_agent_item" | "wave_item" => wave_station(data),
                _ => None,
            }
        })
        .collect()
}

pub fn trailer(value: &Value) -> Vec<Track> {
    tracks(result(value).at("trailer").list("tracks"))
}

pub fn station(value: &Value) -> Option<Station> {
    let value = value.unwrap("station");
    let id = value.at("id");
    let (category, tag) = (id.text("type"), id.text("tag"));
    if category.is_empty() || tag.is_empty() {
        return None;
    }
    Some(Station {
        seed: format!("{category}:{tag}"),
        name: value.text("name").to_owned(),
        category: category.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tracks_with_numeric_and_string_ids() {
        let tracks = tracks(result(&fixture("tracks")).as_array().unwrap());
        assert_eq!(tracks.len(), 2);
        let track = &tracks[0];
        assert_eq!(track.id, "1710808");
        assert!(!track.title.is_empty() && !track.artist.is_empty());
        assert!(!track.album_id.is_empty() && track.duration_ms > 0);
        assert!(track.cover.starts_with("https://"));
        assert_eq!(track.loudness, Some(Loudness { integrated: -9.5, peak: 0.25 }));
        assert_eq!(tracks[1].loudness, None);
    }

    #[test]
    fn reads_similar_entities_and_trailers() {
        let similar = similar_entities(&fixture("album_similar"));
        assert!(
            matches!(&similar[0], Item::Station(station) if station.seed == "artist:82939" && station.name == "Pink Floyd")
        );
        assert!(matches!(&similar[1], Item::Album(album) if album.title == "Trespass"));
        assert!(matches!(&similar[2], Item::Artist(_)));
        let trailer = trailer(&fixture("artist_trailer"));
        assert_eq!(trailer.len(), 2);
        assert_eq!(trailer[0].title, "Nothing Else Matters");
    }

    #[test]
    fn reads_wave_pins_and_upcoming_albums() {
        let pinned =
            wave_station(&serde_json::json!({ "seeds": ["genre:rock", "settingMoodEnergy:fun"], "title": "Rock" }));
        assert!(matches!(pinned, Some(Item::Station(station)) if station.seed == "genre:rock,settingMoodEnergy:fun"));
        assert!(wave_station(&serde_json::json!({ "title": "Nothing" })).is_none());
        let upcoming =
            album(&serde_json::json!({ "id": 1, "title": "Soon", "releaseDate": "2999-01-01T00:00:00+03:00" }))
                .unwrap();
        assert!(upcoming.upcoming && upcoming.release_date == "2999-01-01");
        let released = album(&serde_json::json!({ "id": 2, "releaseDate": "2001-01-01T00:00:00+03:00", "labels": [{ "id": 5, "name": "DMGE" }] })).unwrap();
        assert!(!released.upcoming && released.label_id == "5" && released.label == "DMGE");
    }

    #[test]
    fn reads_chart_positions_and_episode_details() {
        let entry = serde_json::json!({
            "chart": { "position": 4, "progress": "down", "shift": -3 },
            "track": { "id": 1, "title": "One", "isrc": "USUM71703861", "pubDate": "2026-09-25" }
        });
        let track = track(&entry).unwrap();
        assert_eq!(track.chart, Some(ChartPosition { position: 4, trend: Trend::Down(3) }));
        assert_eq!((track.isrc.as_str(), track.published.as_str()), ("USUM71703861", "2026-09-25"));
        let fresh = serde_json::json!({ "chart": { "position": 1, "progress": "new", "shift": 0 }, "track": { "id": 2, "title": "Two" } });
        assert_eq!(super::track(&fresh).unwrap().chart.unwrap().trend, Trend::New);
        assert!(super::track(&serde_json::json!({ "id": 3, "title": "Three" })).unwrap().chart.is_none());
    }
}
