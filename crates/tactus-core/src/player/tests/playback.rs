use super::*;

#[test]
fn settings_changes_apply_and_clamp() {
    let mut player = player();
    player.change_volume(-150);
    assert_eq!(player.settings.volume, 0);
    player.set_speed(1.37);
    assert!((player.settings.speed - 1.35).abs() < f32::EPSILON);
    player.set_repeat(Repeat::One);
    let events = player.take_events();
    assert!(events.contains(&Event::Volume) && events.contains(&Event::Modes));
}

#[test]
fn advances_when_a_track_finishes_and_repeats_one() {
    let (mut player, output) = player_with(Settings::default());
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    assert_eq!(output.0.borrow().starts.len(), 1);
    assert!(output.0.borrow().playing);
    finish_track(&mut player, &output);
    assert_eq!(player.track.as_ref().unwrap().id, "2");
    assert_eq!(output.0.borrow().starts.len(), 2);
    player.set_repeat(Repeat::One);
    finish_track(&mut player, &output);
    assert_eq!(player.track.as_ref().unwrap().id, "2");
    assert_eq!(output.0.borrow().restarts, 1);
    player.set_repeat(Repeat::Off);
    player.settings.continue_with_wave = false;
    let events = finish_track(&mut player, &output);
    assert!(!player.playing && events.contains(&Event::Playback));
    player.settings.continue_with_wave = true;
    player.set_playing(true);
    settle(&mut player);
    finish_track(&mut player, &output);
    assert!(player.wave.is_active() && player.queue.ids().contains(&"w1".to_owned()));
}

#[test]
fn crossfades_into_the_prefetched_track() {
    let settings = Settings { crossfade: 5, ..Settings::default() };
    let (mut player, output) = player_with(settings);
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    at(&output, 160);
    settle(&mut player);
    assert!(player.prefetched.is_some());
    at(&output, 170);
    settle(&mut player);
    assert_eq!(player.queue.index(), 0);
    at(&output, 176);
    settle(&mut player);
    assert_eq!(player.queue.index(), 1);
    let last = *output.0.borrow().starts.last().unwrap();
    assert_eq!(last.fade, Duration::from_secs(5));
}

#[test]
fn normalizes_loudness_when_enabled() {
    let mut loud = track("loud");
    loud.loudness = Some(Loudness { integrated: -8.0, peak: 0.0 });
    let (mut player, output) = player_with(Settings::default());
    player.play_tracks(vec![loud.clone()], 0, "test");
    settle(&mut player);
    assert!(output.0.borrow().starts[0].gain < 0.6);
    let (mut player, output) = player_with(Settings { normalize: false, ..Settings::default() });
    player.play_tracks(vec![loud], 0, "test");
    settle(&mut player);
    assert_eq!(output.0.borrow().starts[0].gain, 1.0);
}

#[test]
fn sleep_timers_stop_playback() {
    let (mut player, output) = player_with(Settings::default());
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    player.set_sleep(Some(SleepTimer::EndOfTrack));
    let events = finish_track(&mut player, &output);
    assert!(!player.playing && player.sleep_timer().is_none());
    assert!(events.contains(&Event::Notice(Notice::SleepStopped)));
    assert_eq!(player.track.as_ref().unwrap().id, "1");
    player.set_playing(true);
    player.set_sleep(Some(SleepTimer::Minutes(0)));
    settle(&mut player);
    assert_eq!(output.0.borrow().fades, [sleep::FADE]);
}

#[test]
fn equalizer_changes_reach_the_output() {
    let (mut player, output) = player_with(Settings::default());
    let gains = audio::Preset::Rock.gains();
    player.update_settings(|settings| settings.equalizer_gains = gains);
    assert_eq!(output.0.borrow().gains, [0.0; audio::BANDS]);
    player.update_settings(|settings| settings.equalizer = true);
    assert_eq!(output.0.borrow().gains, gains);
}

#[test]
fn spoken_content_keeps_its_own_speed() {
    let (mut player, output) = player_with(Settings { speed: 1.5, spoken_speed: 1.0, ..Settings::default() });
    let mut episode = track("ep");
    episode.episode = true;
    player.play_tracks(vec![track("song"), episode], 0, "test");
    settle(&mut player);
    assert_eq!((player.speed(), output.0.borrow().speed), (1.5, 1.5));
    finish_track(&mut player, &output);
    assert_eq!((player.speed(), output.0.borrow().speed), (1.0, 1.0));
    player.change_speed(0.25);
    assert_eq!((player.settings.spoken_speed, player.settings.speed), (1.25, 1.5));
    assert_eq!(output.0.borrow().speed, 1.25);
}

#[test]
fn downloaded_tracks_play_from_the_local_file() {
    let urls: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    let seen = urls.clone();
    let loader: Loader = Arc::new(move |source| {
        seen.lock().unwrap().push(source.url.clone());
        Ok(Loaded::from_source(Box::new(rodio::source::Empty::new())))
    });
    let output = MockOutput::default();
    let mut player = Player::with_output(Settings::default(), |_| Ok(fake().clone()), Box::new(output), loader);
    player.persist = false;
    player.service = Some(fake().clone());
    let file = std::env::temp_dir().join(format!("tactus-local-{}.mp3", std::process::id()));
    std::fs::write(&file, b"ID3").unwrap();
    player.offline.add(&track("local"), &file);
    player.play_tracks(vec![track("local"), track("remote")], 0, "test");
    settle(&mut player);
    player.next();
    settle(&mut player);
    std::fs::remove_file(&file).unwrap();
    let urls = urls.lock().unwrap();
    assert!(urls[0].starts_with("file:") && urls[0].ends_with(".mp3"), "{urls:?}");
    assert!(urls.iter().any(|url| url.starts_with("fake://remote")), "{urls:?}");
}
