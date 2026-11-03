use super::*;

pub const SEARCH_PAGE: usize = 20;

pub fn search_type(kind: SectionKind) -> Option<(&'static str, &'static str)> {
    Some(match kind {
        SectionKind::Tracks => ("track", "tracks"),
        SectionKind::Albums => ("album", "albums"),
        SectionKind::Artists => ("artist", "artists"),
        SectionKind::Playlists => ("playlist", "playlists"),
        SectionKind::Podcasts => ("podcast", "podcasts"),
        SectionKind::Episodes => ("podcast_episode", "podcast_episodes"),
        _ => return None,
    })
}

fn search_items(kind: SectionKind, values: &[Value]) -> Vec<Item> {
    match kind {
        SectionKind::Albums | SectionKind::Podcasts => items(values, album, Item::Album),
        SectionKind::Artists => items(values, artist, Item::Artist),
        SectionKind::Playlists => items(values, playlist, Item::Playlist),
        _ => track_items(values),
    }
}

pub fn search_more(value: &Value, kind: SectionKind, page: u32) -> Paged {
    let Some((_, key)) = search_type(kind) else {
        return Paged::default();
    };
    let block = result(value).at(key);
    let items = search_items(kind, block.list("results"));
    let next = next_page(page, block.num("perPage"), block.num("total"), items.len());
    Paged { items, next }
}

pub fn suggestions(value: &Value) -> Suggestions {
    let value = result(value);
    let best = value.at("best");
    Suggestions {
        best: item_of(best.text("type"), best.at("result")),
        texts: value.list("suggestions").iter().filter_map(Value::as_str).map(str::to_owned).collect(),
    }
}

pub fn search(all: &Value, podcasts: &Value) -> Page {
    let all = result(all);
    let best = item_of(all.at("best").text("type"), all.at("/best/result"));
    let mut sections = Vec::new();
    for kind in [
        SectionKind::Tracks,
        SectionKind::Albums,
        SectionKind::Artists,
        SectionKind::Playlists,
        SectionKind::Podcasts,
        SectionKind::Episodes,
    ] {
        let Some((_, key)) = search_type(kind) else { continue };
        let (block, page) = match kind {
            SectionKind::Podcasts => (result(podcasts).at(key), 0),
            _ => (all.at(key), u32::MAX),
        };
        let items = search_items(kind, block.list("results"));
        let more = match page {
            0 => next_page(0, block.num("perPage"), block.num("total"), items.len()),
            _ => (block.num("total") as usize > items.len()).then_some((items.len() / SEARCH_PAGE) as u32),
        };
        push_paged(&mut sections, kind, "", items, more);
    }
    let mut everything: Vec<Item> = best.clone().into_iter().collect();
    everything.extend(
        sections
            .iter()
            .flat_map(|section| section.items.iter().take(10))
            .filter(|item| best.as_ref() != Some(*item))
            .cloned(),
    );
    if !everything.is_empty() {
        sections.insert(0, Section::new(SectionKind::Best, "", everything));
    }
    Page { title: all.text("text").to_owned(), sections, ..Page::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_with_best_match_first() {
        let page = search(&fixture("search_all"), &fixture("search_podcast"));
        assert_eq!(page.sections[0].kind, SectionKind::Best);
        assert!(matches!(page.sections[0].items[0], Item::Artist(_)));
        let kinds = kinds(&page);
        for kind in [
            SectionKind::Tracks,
            SectionKind::Albums,
            SectionKind::Artists,
            SectionKind::Playlists,
            SectionKind::Podcasts,
            SectionKind::Episodes,
        ] {
            assert!(kinds.contains(&kind), "{kind:?} missing");
        }
        let best = &page.sections[0];
        let artists = best.items.iter().filter(|item| matches!(item, Item::Artist(_))).count();
        assert_eq!(artists, 2);
    }

    #[test]
    fn pages_search_sections() {
        let page = search(&fixture("search_all"), &fixture("search_podcast"));
        assert_eq!(section(&page, SectionKind::Best).unwrap().more, None);
        assert_eq!(section(&page, SectionKind::Tracks).unwrap().more, Some(0));
        assert_eq!(section(&page, SectionKind::Podcasts).unwrap().more, Some(1));
        let next = search_more(&fixture("search_tracks_page"), SectionKind::Tracks, 1);
        assert_eq!(next.items.len(), 2);
        assert_eq!(next.next, Some(2));
        assert_eq!(search_more(&fixture("search_tracks_page"), SectionKind::Best, 1), Paged::default());
    }

    #[test]
    fn parses_suggestions() {
        let suggestions = suggestions(&fixture("suggest"));
        assert!(matches!(suggestions.best, Some(Item::Artist(_))));
        assert_eq!(suggestions.texts.len(), 4);
    }
}
