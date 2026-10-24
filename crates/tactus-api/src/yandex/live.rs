use super::*;

fn report<T>(what: &str, result: Result<T>, size: impl Fn(&T) -> usize) {
    match result {
        Ok(value) => eprintln!("{what}: {}", size(&value)),
        Err(error) => eprintln!("{what}: ERROR {error}"),
    }
}

fn sections(page: &Page) -> usize {
    page.sections.len()
}

#[test]
#[ignore]
fn anonymous_reads() {
    let service = YandexService::new("").unwrap();
    let search = service.page(&View::Search("Metallica".into()), 0).unwrap();
    assert!(!search.sections.is_empty());
    let album = search
        .sections
        .iter()
        .flat_map(|section| &section.items)
        .find_map(|item| match item {
            Item::Album(album) => Some(album.id.clone()),
            _ => None,
        })
        .unwrap();
    let tracks = service.page(&View::Album(album), 0).unwrap().tracks();
    assert!(!tracks.is_empty());
    report("lyrics", service.lyrics(&tracks[0].id), |lyrics| lyrics.as_ref().map_or(0, |lyrics| lyrics.lines.len()));
    for view in [
        View::Radio,
        View::Artist("3121".into()),
        View::Tag("rock".into()),
        View::Track("1710808".into()),
        View::Chart("world".into()),
        View::User("music-blog".into()),
        View::Genre("58e3c0796ebb3476087d190b".into()),
        View::Category("AUDIOBOOKS_MENU".into()),
        View::Category("audibooks_fiction".into()),
        View::Label("6074020".into()),
    ] {
        report(&format!("{view:?}"), service.page(&view, 0), sections);
    }
    report("search page 1", service.more(&View::Search("Metallica".into()), SectionKind::Tracks, 1), |paged| {
        paged.items.len() * 100 + usize::from(paged.next.is_some())
    });
    report("artist tracks page 1", service.more(&View::Artist("3121".into()), SectionKind::Tracks, 1), |paged| {
        paged.items.len()
    });
    report(
        "genre new albums page 1",
        service.more(&View::Genre("58e3c0796ebb3476087d190b".into()), SectionKind::GenreReleases, 1),
        |paged| paged.items.len() * 100 + usize::from(paged.next.is_some()),
    );
    report(
        "playlist by login",
        service.find_playlist(&PlaylistLink::Owner { owner: "yamusic-podcast".into(), kind: 1104 }),
        |key| usize::from(key.owner == 970829816),
    );
    for (view, kind, page) in [
        (View::Genre("58e3c0796ebb3476087d190b".into()), SectionKind::Artists, 1),
        (View::Genre("58e3c0796ebb3476087d190b".into()), SectionKind::Playlists, 0),
        (View::Label("6074020".into()), SectionKind::Albums, 1),
        (View::Artist("3121".into()), SectionKind::Discography, 0),
    ] {
        report(&format!("more {view:?} {kind:?} {page}"), service.more(&view, kind, page), |paged| {
            paged.items.len() * 100 + usize::from(paged.next.is_some())
        });
    }
    report("trailer", service.trailer(&Target::Artist("3121".into())), Vec::len);
    report("suggestions", service.suggestions("metal"), |value| value.texts.len());
    report("track info", service.track_info("1710808"), |value| value.artists.len() * 100 + value.credits.len());
    report("device code", crate::auth::request_device_code(), |value| value.user_code.len());
}

#[test]
#[ignore]
fn signed_in_reads() {
    let saved = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/release/data/token");
    let Some(token) = std::env::var("TACTUS_TOKEN").ok().or_else(|| std::fs::read_to_string(saved).ok()) else {
        eprintln!("no saved sign-in");
        return;
    };
    let service = YandexService::new(token.trim()).unwrap();
    let uid = service.account().unwrap().uid;
    report("library", service.library(uid), |library| library.liked_tracks.len());
    report("settings", service.account_settings(), Vec::len);
    report("playlists", service.playlists(uid), Vec::len);
    for view in [
        View::Waves,
        View::ForYou,
        View::Collection,
        View::Playlists,
        View::Podcasts,
        View::History,
        View::Concerts(String::new()),
        View::Concerts("2".into()),
        View::Album("18031896".into()),
        View::Playlist(PlaylistKey { owner: 103372440, kind: 1565 }),
    ] {
        report(&format!("{view:?}"), service.page(&view, uid), |page| {
            page.sections.len() * 100
                + page
                    .sections
                    .iter()
                    .position(|section| section.kind == SectionKind::Similar)
                    .map_or(0, |index| index + 1)
        });
    }
    for (view, kind, page) in [
        (View::ForYou, SectionKind::NewReleases, 1),
        (View::ForYou, SectionKind::NewPlaylists, 0),
        (View::Podcasts, SectionKind::Podcasts, 1),
    ] {
        report(&format!("more {view:?} {kind:?} {page}"), service.more(&view, kind, page), |paged| {
            paged.items.len() * 100 + usize::from(paged.next.is_some())
        });
    }
    report("concert locations", service.concert_locations(), Vec::len);
    report("signed track info", service.track_info("1710808"), |value| value.artists.len() * 100 + value.credits.len());
    report("world chart positions", service.page(&View::Chart("world".into()), uid), |page| {
        page.tracks().iter().filter(|track| track.chart.is_some()).count()
    });
    if let Ok(page) = service.page(&View::ForYou, uid) {
        let kinds: Vec<String> =
            page.sections.iter().map(|section| format!("{:?}:{}", section.kind, section.items.len())).collect();
        eprintln!("for you sections: {}", kinds.join(" "));
    }
    report("artist facts", service.page(&View::Artist("3121".into()), uid), |page| match &page.subject {
        Some(Item::Artist(artist)) => artist.listeners as usize,
        _ => 0,
    });
    report("queue", service.active_queue(), |queue| queue.as_ref().map_or(0, |queue| queue.track_ids.len()));
    let chart = service.page(&View::ForYou, uid).map(|page| page.tracks()).unwrap_or_default();
    let lyrics = chart.iter().take(10).find_map(|track| service.lyrics(&track.id).ok().flatten());
    eprintln!(
        "lyrics: {} lines, synced {}",
        lyrics.as_ref().map_or(0, |lyrics| lyrics.lines.len()),
        lyrics.as_ref().is_some_and(Lyrics::synced)
    );
    report("source", service.audio_source("1710808", Quality::Lossless), |source| source.url.len());
    report("wave", service.start_wave(&["user:onyourwave".into()]), |batch| batch.tracks.len());
}

#[test]
#[ignore]
fn ynison_link() {
    let saved = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/release/data/token");
    let Some(token) = std::env::var("TACTUS_TOKEN").ok().or_else(|| std::fs::read_to_string(saved).ok()) else {
        eprintln!("no saved sign-in");
        return;
    };
    let identity = crate::ynison::Identity { id: "7ac0be5e7e57".into(), title: "Tactus test".into() };
    let link = crate::ynison::Link::start(token.trim().to_owned(), identity);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if let Some(event) = link.take().into_iter().next() {
            eprintln!("ynison event: {event:?}");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    eprintln!("ynison: no event within 10 s");
}
