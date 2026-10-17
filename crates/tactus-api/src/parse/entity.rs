use super::*;

pub fn track(value: &Value) -> Option<Track> {
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
}
