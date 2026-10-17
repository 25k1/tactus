use super::*;

#[test]
fn loads_pages_once_and_reports_failures() {
    let mut player = player();
    let album = View::Album("70".into());
    player.load(&album, false);
    let events = settle(&mut player);
    assert!(events.contains(&Event::Page(album.clone())));
    assert!(matches!(player.page(&album), Some(PageState::Ready(page)) if page.tracks().len() == 3));
    player.load(&album, false);
    assert_eq!(player.busy, 0);
    let missing = View::Album("404".into());
    player.load(&missing, false);
    let events = settle(&mut player);
    assert!(matches!(player.page(&missing), Some(PageState::Failed(_))));
    assert!(events.iter().any(|event| matches!(event, Event::Error(_))));
}

#[test]
fn plays_a_view_and_edits_the_queue() {
    let mut player = player();
    player.play_view(View::Album("70".into()));
    settle(&mut player);
    assert_eq!(player.queue.ids(), ["1", "2", "3"]);
    assert_eq!(player.track.as_ref().unwrap().id, "1");
    player.play_next(tracks(&["x"]));
    player.add_to_queue(tracks(&["z"]));
    assert_eq!(player.queue.ids(), ["1", "x", "2", "3", "z"]);
    player.move_in_queue(4, 0);
    assert_eq!(player.queue.current().unwrap().id, "1");
    player.clear_queue();
    assert_eq!(player.queue.ids(), ["z", "1"]);
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::QueueCleared)));
}

#[test]
fn waves_fill_the_queue_and_refill_on_demand() {
    let mut player = player();
    player.start_wave(vec![MY_WAVE.into()]);
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::WaveStarted)));
    assert!(player.wave.is_active());
    assert_eq!(player.queue.ids(), ["w1", "w2", "w3", "w4", "w5"]);
}

#[test]
fn loads_more_items_until_exhausted() {
    let mut player = player();
    let view = View::Search("paged".into());
    player.load(&view, false);
    settle(&mut player);
    assert!(player.has_more(&view, 0));
    player.load_more(&view, 0);
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::MoreLoaded(2))));
    player.load_more(&view, 0);
    settle(&mut player);
    assert!(!player.has_more(&view, 0));
    let Some(PageState::Ready(page)) = player.page(&view) else { panic!() };
    assert_eq!(page.sections[0].items.len(), 5);
}
