use super::*;

pub const LIST_PAGE: usize = 25;

fn paged_list(value: &Value, key: &str, parse: fn(&Value) -> Option<Item>) -> Paged {
    let value = result(value);
    let items: Vec<Item> = value.list(key).iter().filter_map(parse).collect();
    let pager = value.at("pager");
    Paged { next: next_page(pager.num("page") as u32, pager.num("perPage"), pager.num("total"), items.len()), items }
}

fn album_item(value: &Value) -> Option<Item> {
    album(value).map(Item::Album)
}

fn artist_item(value: &Value) -> Option<Item> {
    artist(value).map(Item::Artist)
}

fn playlist_item(value: &Value) -> Option<Item> {
    playlist(value).map(Item::Playlist)
}

pub fn label_more(value: &Value, kind: SectionKind) -> Paged {
    match kind {
        SectionKind::Artists => paged_list(value, "artists", artist_item),
        _ => paged_list(value, "albums", album_item),
    }
}

pub fn label_page(main: &Value, albums: Option<&Value>, artists: Option<&Value>) -> Page {
    let main = result(main);
    let mut sections = Vec::new();
    for (kind, value) in [(SectionKind::Albums, albums), (SectionKind::Artists, artists)] {
        if let Some(value) = value {
            let paged = label_more(value, kind);
            push_paged(&mut sections, kind, "", paged.items, paged.next);
        }
    }
    Page {
        title: main.text("name").to_owned(),
        description: main.text("description").to_owned(),
        sections,
        ..Page::default()
    }
}

fn offset_list(value: &Value, key: &str, page: u32, parse: fn(&Value) -> Option<Item>) -> Paged {
    let value = result(value);
    let items: Vec<Item> = value.list(key).iter().filter_map(parse).collect();
    Paged { next: next_page(page, LIST_PAGE as u64, value.at("pager").num("total"), items.len()), items }
}

pub fn genre_artists(value: &Value, page: u32) -> Paged {
    offset_list(value, "artists", page, artist_item)
}

pub fn genre_playlists(value: &Value, page: u32) -> Paged {
    offset_list(value, "playlists", page, playlist_item)
}

pub fn landing_ids(value: &Value, key: &str) -> Vec<String> {
    result(value)
        .list(key)
        .iter()
        .map(|id| id.as_u64().map_or_else(|| id.as_str().unwrap_or_default().to_owned(), |id| id.to_string()))
        .filter(|id| !id.is_empty())
        .collect()
}

pub fn landing_playlists(value: &Value) -> Vec<PlaylistKey> {
    collect(result(value).list("newPlaylists"), |value| {
        Some(PlaylistKey { owner: value.num("uid"), kind: u32::try_from(value.num("kind")).ok()? })
    })
}

pub fn page_of<T: Clone>(all: &[T], page: u32) -> (Vec<T>, Option<u32>) {
    let start = page as usize * LIST_PAGE;
    let slice = all.get(start..).unwrap_or_default();
    let taken = slice.iter().take(LIST_PAGE).cloned().collect();
    (taken, (slice.len() > LIST_PAGE).then_some(page + 1))
}

pub fn albums(value: &Value) -> Vec<Item> {
    items(result(value).as_array().map(Vec::as_slice).unwrap_or_default(), album, Item::Album)
}

pub fn concert_page(value: &Value) -> Page {
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Concerts, "", concerts(result(value).list("items")));
    Page { sections, ..Page::default() }
}

pub fn concert_locations(value: &Value) -> Vec<ConcertLocation> {
    collect(result(value).list("locations"), |value| {
        let (id, name) = (value.id("id"), value.text("name"));
        (!id.is_empty() && !name.is_empty()).then(|| ConcertLocation { id, name: name.to_owned() })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_label_pages() {
        let albums = serde_json::json!({ "result": { "albums": fixture("artist_discography")["result"]["albums"].clone(), "pager": { "page": 0, "perPage": 3, "total": 9 } } });
        let artists = serde_json::json!({ "result": { "artists": [{ "id": "972", "name": "Michael Jackson" }], "pager": { "page": 0, "perPage": 100, "total": 1 } } });
        let main = serde_json::json!({ "result": { "id": 6074020, "name": "DMGE", "description": "" } });
        let page = label_page(&main, Some(&albums), Some(&artists));
        assert_eq!(page.title, "DMGE");
        let albums = section(&page, SectionKind::Albums).unwrap();
        assert_eq!((albums.items.len(), albums.more), (3, Some(1)));
        assert_eq!(section(&page, SectionKind::Artists).unwrap().more, None);
    }

    #[test]
    fn pages_through_landing_lists() {
        let ids: Vec<u32> = (0..60).collect();
        assert_eq!(page_of(&ids, 0), ((0..25).collect(), Some(1)));
        assert_eq!(page_of(&ids, 2), ((50..60).collect(), None));
        assert_eq!(page_of(&ids, 9).0, Vec::<u32>::new());
        let value = serde_json::json!({ "result": { "newReleases": [44278018, "44341242"], "newPlaylists": [{ "uid": 103372440, "kind": 1565 }] } });
        assert_eq!(landing_ids(&value, "newReleases"), ["44278018", "44341242"]);
        assert_eq!(landing_playlists(&value), [PlaylistKey { owner: 103372440, kind: 1565 }]);
    }

    #[test]
    fn reads_the_concert_feed() {
        let page = concert_page(&fixture("concerts_feed"));
        let concerts = &section(&page, SectionKind::Concerts).unwrap().items;
        assert_eq!(concerts.len(), 3);
        assert!(
            matches!(&concerts[0], Item::Concert(concert) if concert.city == "Москва" && !concert.place.is_empty())
        );
        let locations = concert_locations(&fixture("concerts_locations"));
        assert_eq!(locations[0], ConcertLocation { id: "213".into(), name: "Москва".into() });
    }
}
