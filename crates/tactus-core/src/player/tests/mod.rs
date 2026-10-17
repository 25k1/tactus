use super::*;
use crate::audio::mock::{self, MockOutput};
use std::sync::{Arc, OnceLock};
use tactus_api::Loudness;
use tactus_api::fake::{FakeService, track, tracks};
use tactus_api::{Artist, Item, Section, SectionKind};

static SERVICE: OnceLock<Arc<FakeService>> = OnceLock::new();

pub(crate) fn fake() -> &'static Arc<FakeService> {
    SERVICE.get_or_init(|| {
        let mut service = FakeService::default();
        service.pages.insert(
            View::Search("paged".into()),
            Page {
                sections: vec![
                    Section::new(SectionKind::Tracks, "", tracks(&["1"]).into_iter().map(Item::Track).collect())
                        .with_more(Some(0)),
                ],
                ..Page::default()
            },
        );
        service.pages.insert(
            View::Album("70".into()),
            Page {
                title: "Album".into(),
                sections: vec![Section::new(
                    SectionKind::Tracks,
                    "",
                    tracks(&["1", "2", "3"]).into_iter().map(Item::Track).collect(),
                )],
                ..Page::default()
            },
        );
        let artist =
            |id: &str, name: &str| Item::Artist(Artist { id: id.into(), name: name.into(), ..Artist::default() });
        let song = |id: &str, title: &str| Item::Track(Track { title: title.into(), ..track(id) });
        service.pages.insert(
            View::Collection,
            Page {
                sections: vec![
                    Section::new(SectionKind::Tracks, "", vec![song("q1", "Bohemian Rhapsody")]),
                    Section::new(SectionKind::Artists, "", vec![artist("7", "Queen")]),
                    Section::new(SectionKind::DislikedArtists, "", vec![artist("8", "Nickelback")]),
                ],
                ..Page::default()
            },
        );
        service.pages.insert(
            View::Search("metallica".into()),
            Page {
                sections: vec![
                    Section::new(SectionKind::Tracks, "", vec![song("m1", "Enter Sandman")]),
                    Section::new(SectionKind::Artists, "", vec![artist("m", "Metallica")]),
                ],
                ..Page::default()
            },
        );
        service.pages.insert(View::Search("nobody".into()), Page::default());
        Arc::new(service)
    })
}

pub(crate) fn player() -> Player {
    player_with(Settings::default()).0
}

pub(crate) fn player_with(settings: Settings) -> (Player, MockOutput) {
    let output = MockOutput::default();
    let mut player = Player::with_output(settings, |_| Ok(fake().clone()), Box::new(output.clone()), mock::loader());
    player.persist = false;
    player.service = Some(fake().clone());
    player.account = Some(Account { uid: 1, ..Account::default() });
    (player, output)
}

pub(crate) fn finish_track(player: &mut Player, output: &MockOutput) -> Vec<Event> {
    player.started = Instant::now() - FINISH_GUARD * 2;
    output.0.borrow_mut().finished = true;
    settle(player)
}

pub(crate) fn at(output: &MockOutput, seconds: u64) {
    output.0.borrow_mut().position = Duration::from_secs(seconds);
}

pub(crate) fn settle(player: &mut Player) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    loop {
        player.poll();
        events.extend(player.take_events());
        if player.busy == 0 || Instant::now() > deadline {
            return events;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

mod content;
mod library;
mod playback;
mod ynison;
