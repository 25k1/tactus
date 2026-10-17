use super::*;

pub const GENRE_PAGE: u32 = 25;

fn genre_title(value: &Value) -> String {
    let title = value.at("title");
    [title.text("fullTitle"), title.text("title"), value.text("title")]
        .into_iter()
        .find(|text| !text.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn genre(value: &Value) -> Option<Genre> {
    Some(Genre { id: non_empty(value.id("id"))?, title: genre_title(value), seed: String::new() })
}

fn trees(value: &Value) -> Vec<(Genre, Vec<Genre>)> {
    result(value)
        .list("trees")
        .iter()
        .filter_map(|tree| {
            let root = genre(tree.at("root").at("metaTag"))?;
            Some((root, tree.list("leaves").iter().filter_map(genre).collect()))
        })
        .collect()
}

pub fn genres(value: &Value) -> Section {
    Section::new(SectionKind::Genres, "", trees(value).into_iter().map(|(root, _)| Item::Genre(root)).collect())
}

pub fn genre_more(value: &Value, page: u32) -> Paged {
    let value = result(value);
    let items = items(value.list("albums"), album, Item::Album);
    let pager = value.at("pager");
    let shown = u64::from(page * GENRE_PAGE) + items.len() as u64;
    let next = (!items.is_empty() && shown < pager.num("total")).then_some(page + 1);
    Paged { items, next }
}

pub fn genre_page(id: &str, main: &Value, popular: &Value, fresh: &Value, tree: Option<&Value>) -> Page {
    let main = result(main);
    let title = genre_title(main);
    let seed = main.text("stationId").to_owned();
    let leaves = tree
        .map(trees)
        .unwrap_or_default()
        .into_iter()
        .find(|(root, _)| root.id == id)
        .map(|(_, leaves)| leaves)
        .unwrap_or_default();
    let (popular, fresh) = (genre_more(popular, 0), genre_more(fresh, 0));
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Genres, "", leaves.into_iter().map(Item::Genre).collect());
    push_paged(&mut sections, SectionKind::Albums, "", popular.items, popular.next);
    push_paged(&mut sections, SectionKind::GenreReleases, "", fresh.items, fresh.next);
    push(&mut sections, SectionKind::Playlists, "", items(main.list("playlists"), playlist, Item::Playlist));
    push(&mut sections, SectionKind::Artists, "", items(main.list("artists"), artist, Item::Artist));
    Page {
        title: title.clone(),
        subject: Some(Item::Genre(Genre { id: id.to_owned(), title, seed })),
        sections,
        ..Page::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_genre_roots() {
        let section = genres(&fixture("metatags"));
        assert_eq!(section.items.len(), 3);
        assert!(matches!(&section.items[0], Item::Genre(pop) if pop.title == "Поп" && pop.seed.is_empty()));
    }

    #[test]
    fn builds_a_genre_page_with_paged_albums() {
        let albums = fixture("metatag_pop_albums");
        let page = genre_page(
            "58e3c0796ebb3476087d190b",
            &fixture("metatag_pop"),
            &albums,
            &albums,
            Some(&fixture("metatags")),
        );
        assert_eq!(page.title, "Поп");
        assert!(matches!(&page.subject, Some(Item::Genre(genre)) if genre.seed == "genre:pop"));
        assert_eq!(section(&page, SectionKind::Genres).unwrap().items.len(), 3);
        assert_eq!(section(&page, SectionKind::Albums).unwrap().more, Some(1));
        assert_eq!(section(&page, SectionKind::Playlists).unwrap().items.len(), 3);
        assert_eq!(genre_more(&albums, 438).next, Some(439));
        assert_eq!(genre_more(&albums, 439).next, None);
    }
}
