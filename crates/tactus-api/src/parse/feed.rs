use super::*;

fn landing_item(entity: &Value) -> Option<Item> {
    let data = if entity.at("data").is_null() { entity } else { entity.at("data") };
    match entity.text("type") {
        "personal-playlist" => playlist(data.at("data")).map(Item::Playlist),
        "playlist" => playlist(data).map(Item::Playlist),
        "album" | "album-chart-item" => album(data).map(Item::Album),
        "chart-item" | "track" | "episode" | "podcast_episode" => track(data).map(Item::Track),
        "last-recently-played" => album(data.at("playedItem")).map(Item::Album),
        "mix-link" => {
            let id = data.text("url").strip_prefix("/tag/")?.trim_end_matches('/');
            Some(Item::Tag(Tag { id: id.to_owned(), title: data.text("title").to_owned() }))
        }
        _ => None,
    }
}

fn block(block: &Value) -> Option<Section> {
    let items: Vec<Item> = block.list("entities").iter().filter_map(landing_item).collect();
    (!items.is_empty()).then(|| Section::new(SectionKind::Other, block.text("title"), items))
}

pub fn blocks(value: &Value) -> Vec<Section> {
    result(value).list("blocks").iter().filter_map(block).collect()
}

const FEED_EVENTS: [(SectionKind, &[&str], &str); 3] = [
    (SectionKind::FollowedReleases, &["new-albums"], "albums"),
    (SectionKind::GenreReleases, &["new-albums-of-favorite-genre"], "albums"),
    (
        SectionKind::RecommendedArtists,
        &["recommended-similar-artists", "recommended-artists-with-artists-from-history"],
        "artists",
    ),
];

pub fn for_you(feed: Option<&Value>, landing: Option<&Value>, chart: Option<&Value>) -> Page {
    let mut sections = Vec::new();
    if let Some(feed) = feed.map(result) {
        let days: Vec<Value> = feed.list("days").iter().flat_map(|day| day.list("tracksToPlay").to_vec()).collect();
        push(&mut sections, SectionKind::Tracks, "", track_items(&days));
        let generated = feed.list("generatedPlaylists").iter().map(|item| item.at("data")).collect::<Vec<_>>();
        push(
            &mut sections,
            SectionKind::Playlists,
            "",
            generated.into_iter().filter_map(playlist).map(Item::Playlist).collect(),
        );
        for (kind, sources, key) in FEED_EVENTS {
            let values: Vec<Value> = feed
                .list("days")
                .iter()
                .flat_map(|day| day.list("events"))
                .filter(|event| sources.contains(&event.text("typeForFrom")))
                .flat_map(|event| event.list(key).iter().cloned())
                .collect();
            let found = match key {
                "albums" => items(&values, album, Item::Album),
                _ => items(&values, artist, Item::Artist),
            };
            push(&mut sections, kind, "", unique(found));
        }
    }
    let chart =
        chart.map(|chart| track_items(result(chart).at("chart").list("tracks"))).filter(|tracks| !tracks.is_empty());
    for raw in landing.map(result).map(|landing| landing.list("blocks")).unwrap_or_default() {
        let Some(mut section) = block(raw) else { continue };
        if let (true, Some(chart)) = (raw.text("type") == "chart", chart.as_ref()) {
            section.items = chart.clone();
        }
        section.items.retain(|item| !sections.iter().any(|existing| existing.items.contains(item)));
        if !section.items.is_empty() {
            sections.push(section);
        }
    }
    sections.push(charts());
    Page { sections, ..Page::default() }
}

pub fn new_episodes(value: &Value) -> Vec<Section> {
    blocks(value).into_iter().map(|section| Section { kind: SectionKind::NewEpisodes, ..section }).collect()
}

pub fn non_music(value: &Value) -> Page {
    Page {
        title: result(value).text("title").to_owned(),
        sections: blocks(value)
            .into_iter()
            .filter(|section| section.items.len() > 1 || !section.title.is_empty())
            .collect(),
        ..Page::default()
    }
}

pub fn waves(value: &Value) -> Page {
    let sections = value
        .list("waves")
        .iter()
        .filter_map(|wave| {
            let items: Vec<Item> = wave
                .list("items")
                .iter()
                .filter_map(|item| {
                    let seeds: Vec<&str> = item.list("seeds").iter().filter_map(Value::as_str).collect();
                    let seed = if seeds.is_empty() { item.text("stationId").to_owned() } else { seeds.join(",") };
                    (!seed.is_empty()).then(|| {
                        Item::Station(Station {
                            seed,
                            name: item.text("title").to_owned(),
                            category: item.text("header").to_owned(),
                        })
                    })
                })
                .collect();
            (!items.is_empty()).then(|| Section::new(SectionKind::Other, wave.text("title"), items))
        })
        .collect();
    Page { sections, ..Page::default() }
}

pub fn radio(dashboard: Option<&Value>, stations: &Value) -> Page {
    let mut sections = Vec::new();
    if let Some(dashboard) = dashboard {
        push(&mut sections, SectionKind::Best, "", items(result(dashboard).list("stations"), station, Item::Station));
    }
    let mut categories: Vec<Section> = Vec::new();
    for station in collect(result(stations).as_array().map(Vec::as_slice).unwrap_or_default(), station) {
        match categories.iter_mut().find(|section| section.title == station.category) {
            Some(section) => section.items.push(Item::Station(station)),
            None => categories.push(Section::new(
                SectionKind::Stations,
                station.category.clone(),
                vec![Item::Station(station)],
            )),
        }
    }
    sections.extend(categories);
    Page { sections, ..Page::default() }
}

pub fn history(value: &Value) -> Page {
    let sections = result(value)
        .list("historyTabs")
        .iter()
        .filter_map(|tab| {
            let values: Vec<Value> = tab
                .list("items")
                .iter()
                .flat_map(|group| group.list("tracks").iter().map(|item| item.unwrap("data").clone()))
                .collect();
            let items = track_items(&values);
            (!items.is_empty()).then(|| Section::new(SectionKind::Other, tab.text("date"), items))
        })
        .collect();
    Page { sections, ..Page::default() }
}

fn charts() -> Section {
    let items = CHART_REGIONS.iter().map(|region| Item::Chart(Chart { region: (*region).to_owned() })).collect();
    Section::new(SectionKind::Charts, "", items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_landing_blocks_into_for_you() {
        let page = for_you(Some(&fixture("feed")), Some(&fixture("landing")), Some(&fixture("chart")));
        assert_eq!(page.sections[0].kind, SectionKind::Tracks);
        let tags =
            page.sections.iter().flat_map(|section| &section.items).filter(|item| matches!(item, Item::Tag(_))).count();
        assert!(tags > 0);
        let chart = page
            .sections
            .iter()
            .find(|section| {
                section.kind == SectionKind::Other && section.items.iter().all(|item| matches!(item, Item::Track(_)))
            })
            .unwrap();
        assert_eq!(chart.items.len(), 2);
    }

    #[test]
    fn parses_releases_and_recommendations_from_feed_events() {
        let page = for_you(Some(&fixture("feed_events")), None, None);
        assert_eq!(section(&page, SectionKind::FollowedReleases).unwrap().items.len(), 2);
        assert_eq!(section(&page, SectionKind::GenreReleases).unwrap().items.len(), 1);
        let artists = &section(&page, SectionKind::RecommendedArtists).unwrap().items;
        assert_eq!(artists.len(), 3);
    }

    #[test]
    fn parses_new_episodes_in_both_entity_shapes() {
        let sections = new_episodes(&fixture("new_episodes"));
        assert_eq!(sections.len(), 2);
        assert!(sections.iter().all(|section| section.kind == SectionKind::NewEpisodes));
        assert!(matches!(&sections[0].items[0], Item::Album(album) if album.kind == AlbumKind::Podcast));
        assert!(matches!(&sections[1].items[0], Item::Track(track) if track.episode));
    }

    #[test]
    fn parses_non_music_catalogue() {
        let page = non_music(&fixture("non_music"));
        assert!(!page.sections.is_empty());
        assert!(page.sections.iter().all(|section| !section.items.is_empty()));
    }

    #[test]
    fn parses_wave_presets() {
        let page = waves(&fixture("waves"));
        assert_eq!(page.sections.len(), 2);
        let Item::Station(station) = &page.sections[0].items[0] else { panic!() };
        assert!(station.seed.contains(':'));
    }

    #[test]
    fn groups_stations_by_category() {
        let page = radio(None, &fixture("stations"));
        assert!(page.sections.len() >= 2);
        assert!(page.sections.iter().all(|section| section.kind == SectionKind::Stations));
    }

    #[test]
    fn parses_history_by_day() {
        assert_eq!(history(&fixture("history")).sections.len(), 2);
    }
}
