use super::*;

#[test]
fn relations_update_optimistically() {
    let mut player = player();
    let target = Target::Track(track("5"));
    player.toggle_relation(target.clone(), Relation::Like);
    assert!(player.has(&target, Relation::Like));
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::Relation { relation: Relation::Like, on: true })));
    player.toggle_relation(target.clone(), Relation::Dislike);
    assert!(player.has(&target, Relation::Dislike));
    assert!(!player.has(&target, Relation::Like));
    settle(&mut player);
}

#[test]
fn creates_playlists_and_adds_tracks() {
    let mut player = player();
    player.create_playlist("  New list ", true, tracks(&["1"]));
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::PlaylistCreated("New list".into()))));
    assert!(events.contains(&Event::Notice(Notice::AddedToPlaylist { title: "New list".into(), count: 1 })));
    assert!(fake().calls().iter().any(|call| call.starts_with("edit 9 Insert")));
}

#[test]
fn undoes_queue_removals_in_place() {
    let mut player = player();
    player.queue.replace(tracks(&["a", "b", "c", "d"]), 2);
    player.remove_from_queue(&[3, 0, 3]);
    assert_eq!(player.queue.ids(), ["b", "c"]);
    assert!(player.take_events().contains(&Event::Notice(Notice::RemovedFromQueue(2))));
    player.undo();
    assert_eq!(player.queue.ids(), ["a", "b", "c", "d"]);
    assert_eq!(player.queue.current().map(|track| track.id.as_str()), Some("c"));
    assert!(!player.can_undo());
    player.undo();
    assert!(player.take_events().contains(&Event::Notice(Notice::NothingToUndo)));
}

#[test]
fn undoing_a_dislike_restores_the_like() {
    let mut player = player();
    let targets = vec![Target::Track(track("u1")), Target::Track(track("u2"))];
    player.set_relation(targets[0].clone(), Relation::Like, true);
    settle(&mut player);
    player.set_relations(targets.clone(), Relation::Dislike, true);
    settle(&mut player);
    assert!(targets.iter().all(|target| player.has(target, Relation::Dislike)));
    player.undo();
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::Undone)));
    assert!(targets.iter().all(|target| !player.has(target, Relation::Dislike)));
    assert!(player.has(&targets[0], Relation::Like) && !player.has(&targets[1], Relation::Like));
}

#[test]
fn undoes_playlist_removals_after_they_succeed() {
    let mut player = player();
    let playlist = Playlist { key: PlaylistKey { owner: 1, kind: 4 }, ..Playlist::default() };
    let removed = vec![(1, track("p1")), (3, track("p3"))];
    player.change_playlist(&playlist, PlaylistEdit::Remove(removed.clone()));
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::RemovedFromPlaylist(2))));
    player.undo();
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::Undone)));
    let restore = format!("edit 4 {:?}", PlaylistEdit::Restore(removed));
    assert!(fake().calls().contains(&restore));
}

#[test]
fn pins_update_optimistically() {
    let mut player = player();
    let album = Item::Album(tactus_api::Album { id: "pin1".into(), ..tactus_api::Album::default() });
    let target = album.target().unwrap();
    player.set_pin(album.clone(), true);
    assert!(player.is_pinned(&target));
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::Pinned(true))));
    assert!(fake().calls().contains(&format!("pin true {target:?}")));
    player.set_pin(album, false);
    assert!(!player.is_pinned(&target));
    settle(&mut player);
}

#[test]
fn uploads_files_into_a_playlist() {
    let mut player = player();
    let file = std::env::temp_dir().join(format!("tactus-upload-{}.mp3", std::process::id()));
    std::fs::write(&file, b"ID3").unwrap();
    let playlist = Playlist { key: PlaylistKey { owner: 1, kind: 6 }, ..Playlist::default() };
    player.upload(&playlist, vec![file.clone()]);
    let events = settle(&mut player);
    std::fs::remove_file(&file).unwrap();
    assert!(events.contains(&Event::Notice(Notice::Uploaded(1))));
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(fake().calls().contains(&format!("upload 6 {name} 3")));
}

#[test]
fn pins_waves_and_toggles_presaves() {
    let mut player = player();
    let wave = Item::Station(tactus_api::Station { seed: "genre:rock".into(), ..tactus_api::Station::default() });
    player.set_pin(wave.clone(), true);
    settle(&mut player);
    assert!(player.is_pinned(&wave.target().unwrap()));
    assert!(fake().calls().contains(&"pin true Wave(\"genre:rock\")".to_owned()));
    player.set_presave("soon".into(), true);
    assert!(player.is_presaved("soon"));
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::Presaved(true))));
    player.set_presave("soon".into(), false);
    settle(&mut player);
    assert!(!player.is_presaved("soon"));
    assert!(fake().calls().contains(&"presave soon false".to_owned()));
}

#[test]
fn joins_collective_playlists_and_plays_trailers() {
    let mut player = player();
    player.join_playlist(tactus_api::PlaylistLink::Owner { owner: "42".into(), kind: 7 }, "secret".into());
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::JoinedPlaylist)));
    assert!(events.contains(&Event::Open(View::Playlist(PlaylistKey { owner: 42, kind: 7 }))));
    assert!(fake().calls().contains(&"join 42 secret".to_owned()));
    player.play_trailer(Target::Artist("3121".into()));
    settle(&mut player);
    assert_eq!(player.queue.ids(), ["t1", "t2"]);
}
