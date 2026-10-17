use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Original,
    Title,
    Artist,
    Duration,
    Year,
}

impl Sort {
    pub const ALL: [Sort; 5] = [Sort::Original, Sort::Title, Sort::Artist, Sort::Duration, Sort::Year];

    pub fn label(self) -> String {
        match self {
            Sort::Original => fl!("sort-original"),
            Sort::Title => fl!("sort-title"),
            Sort::Artist => fl!("sort-artist"),
            Sort::Duration => fl!("sort-duration"),
            Sort::Year => fl!("sort-year"),
        }
    }
}

fn year(entry: &Entry) -> u32 {
    match entry {
        Entry::Item(Item::Album(album)) => album.year,
        entry => entry.track().map_or(0, |track| track.year),
    }
}

pub fn arrange(entries: &[Entry], filter: &str, sort: Sort) -> Vec<Row> {
    let needle = filter.trim().to_lowercase();
    let keyed = !needle.is_empty() || matches!(sort, Sort::Title | Sort::Artist);
    let mut rows: Vec<(usize, [String; 4])> = entries
        .iter()
        .enumerate()
        .map(|(origin, entry)| {
            (origin, if keyed { columns(entry).map(|text| text.to_lowercase()) } else { Default::default() })
        })
        .filter(|(_, texts)| needle.is_empty() || texts[..3].iter().any(|text| text.contains(&needle)))
        .collect();
    let duration = |origin: usize| entries[origin].track().map_or(0, |track| track.duration_ms);
    match sort {
        Sort::Original => {}
        Sort::Title => rows.sort_by(|(_, a), (_, b)| a[0].cmp(&b[0])),
        Sort::Artist => rows.sort_by(|(_, a), (_, b)| a[1].cmp(&b[1]).then_with(|| a[0].cmp(&b[0]))),
        Sort::Duration => rows.sort_by_key(|(origin, _)| std::cmp::Reverse(duration(*origin))),
        Sort::Year => rows.sort_by_key(|(origin, _)| std::cmp::Reverse(year(&entries[*origin]))),
    }
    rows.into_iter().map(|(origin, _)| Row { origin, entry: entries[origin].clone() }).collect()
}

fn album_kind(kind: AlbumKind) -> String {
    match kind {
        AlbumKind::Music => fl!("kind-album"),
        AlbumKind::Single => fl!("kind-single"),
        AlbumKind::Podcast => fl!("kind-podcast"),
        AlbumKind::Audiobook => fl!("kind-audiobook"),
    }
}

pub fn download_state(state: &DownloadState) -> String {
    match state {
        DownloadState::Queued => fl!("download-queued"),
        DownloadState::Downloading(percent) => fl!("download-progress", percent = u32::from(*percent)),
        DownloadState::Finished(_) => fl!("download-finished"),
        DownloadState::Failed(error) => fl!("download-failed", error = error.clone()),
    }
}

pub fn columns(entry: &Entry) -> [String; 4] {
    let join = |parts: Vec<String>| join_present(parts, ", ");
    match entry {
        Entry::Item(Item::Track(track)) => [
            track.full_title(),
            track.artist.clone(),
            join(vec![
                track.album.clone(),
                if track.year > 0 { track.year.to_string() } else { String::new() },
                if track.explicit { fl!("track-explicit") } else { String::new() },
                if track.played { fl!("track-played") } else { String::new() },
                if track.downloaded { fl!("track-downloaded") } else { String::new() },
            ]),
            format_time(track.duration_ms),
        ],
        Entry::Item(Item::Album(album)) => [
            album.title.clone(),
            album.artist.clone(),
            join(vec![
                album_kind(album.kind),
                if album.year > 0 { album.year.to_string() } else { String::new() },
                if album.track_count > 0 { fl!("tracks-count", count = album.track_count) } else { String::new() },
            ]),
            String::new(),
        ],
        Entry::Item(Item::Artist(artist)) => {
            [artist.name.clone(), artist.genres.clone(), fl!("kind-artist"), String::new()]
        }
        Entry::Item(Item::Playlist(playlist)) => [
            playlist.title.clone(),
            playlist.owner.clone(),
            join(vec![
                fl!("kind-playlist"),
                fl!("tracks-count", count = playlist.track_count),
                if playlist.collective { fl!("playlist-collective") } else { String::new() },
            ]),
            String::new(),
        ],
        Entry::Item(Item::Station(station)) => {
            [station.name.clone(), station_category(&station.category), fl!("kind-station"), String::new()]
        }
        Entry::Item(Item::Tag(tag)) => [tag.title.clone(), String::new(), fl!("kind-collection"), String::new()],
        Entry::Item(Item::Genre(genre)) => [genre.title.clone(), String::new(), fl!("kind-genre"), String::new()],
        Entry::Item(Item::Link(link)) => [link.title.clone(), link.url.clone(), fl!("kind-link"), String::new()],
        Entry::Item(Item::Chart(chart)) => {
            [chart_label(&chart.region), String::new(), fl!("kind-chart"), String::new()]
        }
        Entry::Download(download) => [
            download.track.full_title(),
            download.track.artist.clone(),
            download_state(&download.state),
            format_time(download.track.duration_ms),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn filters_and_sorts_rows_keeping_origins() {
        let mut first = track("1");
        first.title = "Zebra".into();
        first.duration_ms = 1_000;
        let mut second = track("2");
        second.title = "apple".into();
        second.duration_ms = 9_000;
        let entries = vec![Entry::Item(Item::Track(first)), Entry::Item(Item::Track(second))];
        let origins = |rows: Vec<Row>| rows.into_iter().map(|row| row.origin).collect::<Vec<_>>();
        assert_eq!(origins(arrange(&entries, "", Sort::Original)), [0, 1]);
        assert_eq!(origins(arrange(&entries, "", Sort::Title)), [1, 0]);
        assert_eq!(origins(arrange(&entries, "", Sort::Duration)), [1, 0]);
        assert_eq!(origins(arrange(&entries, " ZEB ", Sort::Original)), [0]);
        assert_eq!(origins(arrange(&entries, "artist", Sort::Original)), [0, 1]);
        assert!(arrange(&entries, "nothing", Sort::Original).is_empty());
    }

    fn entries() -> impl Strategy<Value = Vec<Entry>> {
        proptest::collection::vec(("[a-zA-Zа-яА-Я ]{0,8}", "[a-z]{0,6}", 0u64..600_000, 1950u32..2030), 0..30).prop_map(
            |specs| {
                specs
                    .into_iter()
                    .enumerate()
                    .map(|(index, (title, artist, duration_ms, year))| {
                        let mut track = track(&index.to_string());
                        (track.title, track.artist, track.duration_ms, track.year) = (title, artist, duration_ms, year);
                        Entry::Item(Item::Track(track))
                    })
                    .collect()
            },
        )
    }

    fn sort() -> impl Strategy<Value = Sort> {
        proptest::sample::select(Sort::ALL.to_vec())
    }

    proptest! {
        #[test]
        fn arranging_keeps_valid_unique_origins(entries in entries(), filter in "[a-zа-я ]{0,3}", sort in sort()) {
            let rows = arrange(&entries, &filter, sort);
            let mut origins: Vec<usize> = rows.iter().map(|row| row.origin).collect();
            prop_assert!(rows.iter().all(|row| entries.get(row.origin) == Some(&row.entry)));
            origins.sort_unstable();
            origins.dedup();
            prop_assert_eq!(origins.len(), rows.len());
            let needle = filter.trim().to_lowercase();
            for (origin, entry) in entries.iter().enumerate() {
                let matches = columns(entry)[..3].iter().any(|text| text.to_lowercase().contains(&needle));
                prop_assert_eq!(origins.binary_search(&origin).is_ok(), matches);
            }
        }

        #[test]
        fn unfiltered_lists_are_permutations_in_order(entries in entries(), sort in sort()) {
            let rows = arrange(&entries, "", sort);
            prop_assert_eq!(rows.len(), entries.len());
            let titles: Vec<String> = rows.iter().map(|row| columns(&row.entry)[0].to_lowercase()).collect();
            match sort {
                Sort::Original => prop_assert!(rows.iter().enumerate().all(|(index, row)| row.origin == index)),
                Sort::Title => prop_assert!(titles.is_sorted()),
                Sort::Duration => {
                    let durations: Vec<u64> = rows.iter().filter_map(|row| row.entry.track()).map(|track| track.duration_ms).collect();
                    prop_assert!(durations.is_sorted_by(|a, b| a >= b));
                }
                Sort::Artist | Sort::Year => {}
            }
        }
    }

    #[test]
    fn columns_describe_items() {
        let mut track = track("1");
        track.version = "Live".into();
        let columns = columns(&Entry::Item(Item::Track(track)));
        assert_eq!(columns, ["Track 1 (Live)", "Artist", "Album", "3:00"].map(String::from));
        let album = columns_for_album();
        assert!(album[2].contains("2001"));
    }

    fn columns_for_album() -> [String; 4] {
        columns(&Entry::Item(Item::Album(Album { year: 2001, track_count: 3, ..Album::default() })))
    }
}
