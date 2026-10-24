use super::*;
use tactus_api::Relation;

#[test]
fn my_wave_uses_the_saved_settings() {
    let mut player = player();
    assert_eq!(player.my_wave(), [MY_WAVE]);
    player.set_wave_seeds(vec!["settingLanguage:russian".into()]);
    assert_eq!(player.my_wave(), [MY_WAVE, "settingLanguage:russian"]);
    player.start_my_wave();
    settle(&mut player);
    assert!(fake().calls().contains(&"wave user:onyourwave,settingLanguage:russian".to_owned()));
    player.set_wave_seeds(vec!["settingDiversity:discover".into()]);
    settle(&mut player);
    assert_eq!(player.wave.seeds, [MY_WAVE, "settingDiversity:discover"]);
}

#[test]
fn loads_wave_options_and_plays_the_last_wave() {
    let mut player = player();
    player.load_wave_options();
    settle(&mut player);
    assert_eq!(player.wave_options().unwrap().settings[0].key, "language");
    player.play_last_wave();
    settle(&mut player);
    assert_eq!(player.wave.seeds, ["genre:rock"]);
    player.reset_last_wave();
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::LastWaveReset)));
}

#[test]
fn likes_in_a_wave_reach_the_session() {
    let mut player = player();
    player.start_wave(vec!["genre:jazz".into()]);
    settle(&mut player);
    let track = player.queue.tracks()[1].clone();
    player.set_relation(Target::Track(track), Relation::Like, true);
    settle(&mut player);
    assert!(fake().calls().iter().any(|call| call == "feedback session Like { track_id: \"w2\" }"));
}

#[test]
fn generated_music_loops_and_ignores_track_actions() {
    let (mut player, output) = player_with(Settings::default());
    player.start_wave(vec!["generative:focus".into()]);
    settle(&mut player);
    let track = player.track.clone().unwrap();
    assert_eq!((track.id.as_str(), track.title.as_str()), ("generative:focus", "Inspiration"));
    assert!(player.is_generated_playing());
    assert!(fake().calls().iter().any(|call| call == "stream generative:focus stream-generative:focus streamStarted"));
    player.set_playing(false);
    settle(&mut player);
    assert!(fake().calls().iter().any(|call| call.ends_with("streamPause")));
    player.set_playing(true);
    player.next();
    player.previous();
    assert_eq!(player.queue.ids(), ["generative:focus"]);
    finish_track(&mut player, &output);
    assert_eq!(player.track.as_ref().unwrap().id, "generative:focus");
    assert!(player.playing);
    player.set_relation(Target::Track(track), Relation::Like, true);
    assert!(!fake().calls().iter().any(|call| call.contains("Like true") && call.contains("generative")));
}
