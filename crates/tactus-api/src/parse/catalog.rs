use super::*;

pub const ARTIST_PAGE: u32 = 100;

pub fn artist_more(value: &Value, kind: SectionKind) -> Paged {
    let value = result(value);
    let items = match kind {
        SectionKind::Albums => items(value.list("albums"), album, Item::Album),
        _ => track_items(value.list("tracks")),
    };
    let pager = value.at("pager");
    let next = next_page(pager.num("page") as u32, pager.num("perPage"), pager.num("total"), items.len());
    Paged { items, next }
}

pub fn album_page(value: &Value) -> Page {
    let value = result(value);
    let subject = album(value);
    let volumes: Vec<Value> =
        value.list("volumes").iter().flat_map(|volume| volume.as_array().cloned().unwrap_or_default()).collect();
    let kind = match subject.as_ref().map(|album| album.kind) {
        Some(AlbumKind::Podcast | AlbumKind::Audiobook) => SectionKind::Episodes,
        _ => SectionKind::Tracks,
    };
    let mut sections = Vec::new();
    push(&mut sections, kind, "", track_items(&volumes));
    push(&mut sections, SectionKind::Editions, "", items(value.list("duplicates"), album, Item::Album));
    Page {
        title: value.text("title").to_owned(),
        description: [value.text("description"), value.text("shortDescription")]
            .into_iter()
            .find(|text| !text.is_empty())
            .unwrap_or_default()
            .to_owned(),
        subject: subject.map(Item::Album),
        sections,
    }
}

pub fn artist_page(brief: &Value, tracks: Option<&Value>, albums: Option<&Value>) -> Page {
    let brief = result(brief);
    let artist_value = brief.at("artist");
    let paged = |value: Option<&Value>, kind: SectionKind, fallback: Vec<Item>| {
        value
            .map(|value| artist_more(value, kind))
            .filter(|paged| !paged.items.is_empty())
            .unwrap_or(Paged { items: fallback, next: None })
    };
    let tracks = paged(tracks, SectionKind::Tracks, track_items(brief.list("popularTracks")));
    let albums = paged(albums, SectionKind::Albums, items(brief.list("albums"), album, Item::Album));
    let mut sections = Vec::new();
    push_paged(&mut sections, SectionKind::Tracks, "", tracks.items, tracks.next);
    push_paged(&mut sections, SectionKind::Albums, "", albums.items, albums.next);
    push(&mut sections, SectionKind::AppearsOn, "", items(brief.list("alsoAlbums"), album, Item::Album));
    push(&mut sections, SectionKind::SimilarArtists, "", items(brief.list("similarArtists"), artist, Item::Artist));
    push(&mut sections, SectionKind::Playlists, "", items(brief.list("playlists"), playlist, Item::Playlist));
    push(&mut sections, SectionKind::Links, "", items(artist_value.list("links"), link, Item::Link));
    Page {
        title: artist_value.text("name").to_owned(),
        description: artist_value.at("description").text("text").to_owned(),
        subject: artist(artist_value)
            .map(|artist| Artist { listeners: brief.at("stats").num("lastMonthListeners"), ..artist })
            .map(Item::Artist),
        sections,
    }
}

pub fn playlist_page(value: &Value, suggestions: Option<&Value>) -> Page {
    let value = result(value);
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Tracks, "", track_items(value.list("tracks")));
    if let Some(suggestions) = suggestions {
        push(&mut sections, SectionKind::Suggestions, "", track_items(result(suggestions).list("tracks")));
    }
    Page {
        title: value.text("title").to_owned(),
        description: value.text("description").to_owned(),
        subject: playlist(value).map(Item::Playlist),
        sections,
    }
}

pub fn playlists(value: &Value) -> Vec<Playlist> {
    collect(result(value).as_array().map(Vec::as_slice).unwrap_or_default(), playlist)
}

pub fn tag_ids(value: &Value) -> (String, Vec<PlaylistKey>) {
    let value = result(value);
    (value.at("tag").text("name").to_owned(), collect(value.list("ids"), playlist_key))
}

fn link(value: &Value) -> Option<Link> {
    let url = value.text("href");
    (!url.is_empty()).then(|| Link {
        title: match value.text("title") {
            "" => url.to_owned(),
            title => title.to_owned(),
        },
        url: url.to_owned(),
    })
}

fn versions(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(tracks) => tracks.clone(),
        Value::Object(groups) => groups.values().filter_map(Value::as_array).flatten().cloned().collect(),
        _ => Vec::new(),
    }
}

pub fn track_page(full: &Value, similar: Option<&Value>) -> Page {
    let full = result(full);
    let subject = track(full.at("track"));
    let similar = similar
        .map(|value| track_items(result(value).list("similarTracks")))
        .filter(|tracks| !tracks.is_empty())
        .unwrap_or_else(|| track_items(full.list("similarTracks")));
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Similar, "", similar);
    push(&mut sections, SectionKind::Versions, "", track_items(&versions(full.at("otherVersions"))));
    Page {
        title: subject.as_ref().map(Track::full_title).unwrap_or_default(),
        subject: subject.map(Item::Track),
        sections,
        ..Page::default()
    }
}

pub fn chart_page(value: &Value) -> Page {
    let value = result(value);
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Tracks, "", track_items(value.at("chart").list("tracks")));
    Page { description: value.text("title").to_owned(), sections, ..Page::default() }
}

pub fn user_page(login: &str, value: &Value) -> Page {
    let playlists = playlists(value);
    let title = playlists.first().map_or_else(|| login.to_owned(), |playlist| playlist.owner.clone());
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Playlists, "", playlists.into_iter().map(Item::Playlist).collect());
    Page { title, sections, ..Page::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_artist_tracks_and_extends_without_duplicates() {
        let paged = artist_more(&fixture("artist_tracks"), SectionKind::Tracks);
        assert_eq!(paged.next, Some(1));
        let mut section = Section::new(SectionKind::Tracks, "", paged.items.clone()).with_more(Some(1));
        let mut more = paged.clone();
        more.items.push(Item::Track(crate::fake::track("new")));
        more.next = None;
        assert_eq!(section.extend(more), 1);
        assert_eq!(section.more, None);
    }

    #[test]
    fn parses_album_with_tracks() {
        let page = album_page(&fixture("album_with_tracks"));
        assert!(matches!(page.subject, Some(Item::Album(_))));
        assert_eq!(page.tracks().len(), 3);
    }

    #[test]
    fn parses_artist() {
        let page =
            artist_page(&fixture("artist_brief"), Some(&fixture("artist_tracks")), Some(&fixture("artist_albums")));
        assert_eq!(page.title, "Metallica");
        let Some(Item::Artist(metallica)) = &page.subject else { panic!("no artist subject") };
        assert_eq!((metallica.listeners, metallica.since), (2_870_852, 1981));
        assert!(metallica.likes > 0 && !metallica.countries.is_empty());
        assert_eq!(section(&page, SectionKind::Tracks).unwrap().items.len(), 2);
        assert_eq!(section(&page, SectionKind::Albums).unwrap().items.len(), 2);
        assert!(section(&page, SectionKind::SimilarArtists).is_some());
        let fallback = artist_page(&fixture("artist_brief"), None, None);
        assert!(!section(&fallback, SectionKind::Tracks).unwrap().items.is_empty());
    }

    #[test]
    fn parses_playlist_and_suggestions() {
        let page = playlist_page(&fixture("playlist"), Some(&fixture("playlist_recommendations")));
        let Some(Item::Playlist(playlist)) = &page.subject else { panic!("no playlist") };
        assert_eq!(playlist.key, PlaylistKey { owner: 103372440, kind: 2131 });
        assert!(playlist.public);
        assert_eq!(section(&page, SectionKind::Tracks).unwrap().items.len(), 2);
        assert_eq!(section(&page, SectionKind::Suggestions).unwrap().items.len(), 2);
        assert_eq!(page.tracks().len(), 2);
    }

    #[test]
    fn parses_tags_and_similar() {
        let (name, keys) = tag_ids(&fixture("tag"));
        assert!(!name.is_empty());
        assert_eq!(keys.len(), 8);
        let page = track_page(&fixture("full_info"), Some(&fixture("similar")));
        assert!(matches!(page.subject, Some(Item::Track(_))));
        assert_eq!(section(&page, SectionKind::Similar).unwrap().items.len(), 2);
        let tracks = result(&fixture("tracks")).clone();
        let full = serde_json::json!({ "track": tracks[0], "otherVersions": { "live": [tracks[1]] } });
        let page = track_page(&full, None);
        assert_eq!(section(&page, SectionKind::Versions).unwrap().items.len(), 1);
    }

    #[test]
    fn parses_editions_links_and_charts() {
        let album = album_page(&fixture("album_with_tracks"));
        assert_eq!(section(&album, SectionKind::Editions).unwrap().items.len(), 1);
        let artist = artist_page(&fixture("artist_brief"), None, None);
        let links = section(&artist, SectionKind::Links).unwrap();
        assert!(matches!(&links.items[0], Item::Link(link) if link.url.starts_with("http")));
        assert_eq!(section(&artist, SectionKind::Playlists).unwrap().items.len(), 2);
        let for_you = for_you(None, None, None);
        assert_eq!(section(&for_you, SectionKind::Charts).unwrap().items.len(), CHART_REGIONS.len());
        assert_eq!(chart_page(&fixture("chart")).tracks().len(), 2);
        let user = user_page("someone", &fixture("playlists_list"));
        assert_eq!(section(&user, SectionKind::Playlists).unwrap().items.len(), 2);
        assert_ne!(user.title, "someone");
    }
}
