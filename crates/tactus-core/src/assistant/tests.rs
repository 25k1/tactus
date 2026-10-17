use super::intents::mood_seed;
use super::*;
use crate::player::SleepTimer;
use crate::player::queue::Repeat;
use crate::player::tests::{player, settle};
use crate::player::{Event, Notice};
use tactus_api::fake::tracks;
use tactus_api::{Playlist, PlaylistKey};
use tactus_api::{Relation, Target};
use tactus_voice::{Language, Mood};

fn assistant() -> Assistant {
    Assistant { interface: "en".into(), phrases: Some(Phrases::builtin(Language::English)), ..Assistant::default() }
}

fn say(assistant: &mut Assistant, player: &mut Player, text: &str) -> Vec<Outcome> {
    assistant.text(player, text, Instant::now())
}

fn understood(outcome: Option<Outcome>) -> Vec<Outcome> {
    std::iter::once(Outcome::Cue(Cue::Understood)).chain(outcome).collect()
}

fn with_library() -> Player {
    let mut player = player();
    player.load(&View::Collection, false);
    settle(&mut player);
    player
}

#[test]
fn controls_playback_and_asks_for_feedback() {
    let (mut assistant, mut player) = (assistant(), player());
    let feedback = |kind| Some(Outcome::Feedback(kind));
    assert_eq!(say(&mut assistant, &mut player, "volume to 30"), understood(feedback(Announcement::Volume)));
    assert_eq!(player.settings.volume, 30);
    assert_eq!(say(&mut assistant, &mut player, "faster"), understood(Some(Outcome::Speed)));
    assert_eq!(say(&mut assistant, &mut player, "repeat all"), understood(feedback(Announcement::Modes)));
    assert_eq!(player.settings.repeat, Repeat::All);
    assert_eq!(say(&mut assistant, &mut player, "stop in 20 minutes"), understood(None));
    assert_eq!(player.sleep_timer(), Some(SleepTimer::Minutes(20)));
    assert_eq!(say(&mut assistant, &mut player, "open the queue"), understood(Some(Outcome::Open(Place::Queue))));
    assert_eq!(say(&mut assistant, &mut player, "search for abba"), understood(Some(Outcome::Search("abba".into()))));
}

#[test]
fn reports_what_it_did_not_understand() {
    let (mut assistant, mut player) = (assistant(), player());
    assert_eq!(
        say(&mut assistant, &mut player, "xyzzy"),
        [Outcome::Cue(Cue::NotUnderstood), Outcome::Reply(Reply::NotUnderstood("xyzzy".into()))]
    );
}

#[test]
fn relations_need_a_playing_track() {
    let (mut assistant, mut player) = (assistant(), player());
    assert_eq!(say(&mut assistant, &mut player, "like this"), understood(Some(Outcome::Reply(Reply::NothingPlaying))));
    player.play_tracks(tracks(&["1"]), 0, "test");
    assert_eq!(say(&mut assistant, &mut player, "like this"), understood(None));
    assert!(player.has(&Target::Track(tracks(&["1"]).remove(0)), Relation::Like));
    settle(&mut player);
}

#[test]
fn confirms_library_matches_before_playing() {
    let (mut assistant, mut player) = (assistant(), with_library());
    let will_play = understood(Some(Outcome::Reply(Reply::WillPlay("Bohemian Rhapsody".into()))));
    assert_eq!(say(&mut assistant, &mut player, "play song bohemian rhapsody"), will_play);
    assert_eq!(say(&mut assistant, &mut player, "no"), understood(Some(Outcome::Reply(Reply::Cancelled))));
    assert_eq!(say(&mut assistant, &mut player, "play bohemian rhapsody"), will_play);
    assert!(assistant.poll(&mut player, Instant::now()).is_empty());
    assert!(player.track.is_none());
    assistant.poll(&mut player, Instant::now() + CONFIRM);
    assert_eq!(player.track.as_ref().map(|track| track.id.as_str()), Some("q1"));
    assert_eq!(
        say(&mut assistant, &mut player, "play artist queen"),
        understood(Some(Outcome::Reply(Reply::WillPlay("Queen".into()))))
    );
    assert_eq!(assistant.press(&mut player, Instant::now()), Some(Outcome::Reply(Reply::Cancelled)));
    assert!(assistant.pending.is_none());
}

#[test]
fn searches_when_the_library_has_no_match() {
    let (mut assistant, mut player) = (assistant(), with_library());
    let view = View::Search("metallica".into());
    assert_eq!(
        say(&mut assistant, &mut player, "play metallica"),
        understood(Some(Outcome::Reply(Reply::Searching("metallica".into()))))
    );
    assert!(assistant.waits_for(&view));
    assert_eq!(assistant.page_ready(&mut player, &View::Collection, Instant::now()), None);
    settle(&mut player);
    assert_eq!(
        assistant.page_ready(&mut player, &view, Instant::now()),
        Some(Outcome::Reply(Reply::WillPlay("Metallica".into())))
    );
    assert!(!assistant.waits_for(&view));
    say(&mut assistant, &mut player, "play artist nickelback");
    assert!(assistant.waits_for(&View::Search("nickelback".into())));
}

#[test]
fn reports_searches_without_results() {
    let (mut assistant, mut player) = (assistant(), with_library());
    say(&mut assistant, &mut player, "play nobody");
    settle(&mut player);
    let view = View::Search("nobody".into());
    assert_eq!(
        assistant.page_ready(&mut player, &view, Instant::now()),
        Some(Outcome::Reply(Reply::NotFound("nobody".into())))
    );
}

#[test]
fn plays_likes_once_the_collection_loads() {
    let (mut assistant, mut player) = (assistant(), player());
    assert_eq!(say(&mut assistant, &mut player, "play my likes"), understood(None));
    assert!(assistant.waits_for(&View::Collection));
    settle(&mut player);
    assert_eq!(assistant.page_ready(&mut player, &View::Collection, Instant::now()), None);
    assert_eq!(player.queue.ids(), ["q1"]);
}

#[test]
fn adds_the_current_track_to_a_named_playlist() {
    let (mut assistant, mut player) = (assistant(), player());
    player.playlists =
        vec![Playlist { key: PlaylistKey { owner: 1, kind: 3 }, title: "Road trip".into(), ..Playlist::default() }];
    player.play_tracks(tracks(&["1"]), 0, "test");
    assert_eq!(say(&mut assistant, &mut player, "add this to playlist road trip"), understood(None));
    let events = settle(&mut player);
    assert!(events.contains(&Event::Notice(Notice::AddedToPlaylist { title: "Road trip".into(), count: 1 })));
    assert_eq!(
        say(&mut assistant, &mut player, "add this to playlist xyzzy"),
        understood(Some(Outcome::Reply(Reply::NoPlaylist("xyzzy".into()))))
    );
}

#[test]
fn pauses_music_while_listening() {
    let (mut assistant, mut player) = (assistant(), player());
    player.settings.voice.music = MusicWhileListening::Pause;
    player.play_tracks(tracks(&["1"]), 0, "test");
    let now = Instant::now();
    assert_eq!(assistant.heard(&mut player, Heard::Listening, now), [Outcome::Cue(Cue::Listening)]);
    assert!(!player.playing);
    assert_eq!(
        assistant.heard(&mut player, Heard::Nothing, now),
        [Outcome::Cue(Cue::NotUnderstood), Outcome::Reply(Reply::Nothing)]
    );
    assert!(player.playing);
}

#[test]
fn press_explains_why_it_cannot_listen() {
    let (mut assistant, mut player) = (assistant(), player());
    assert_eq!(assistant.press(&mut player, Instant::now()), Some(Outcome::Reply(Reply::Off)));
    player.settings.voice.enabled = true;
    assert_eq!(assistant.listen_once(&mut player, Instant::now()), Some(Outcome::OfferDownload));
    assert_eq!(
        assistant.heard(&mut player, Heard::Failed("broken".into()), Instant::now()),
        [Outcome::Reply(Reply::Failed("broken".into()))]
    );
}

#[test]
fn moods_map_to_wave_settings() {
    assert_eq!(mood_seed(Mood::Energetic), "settingMoodEnergy:active");
    assert_eq!(mood_seed(Mood::Calm), "settingMoodEnergy:calm");
}
