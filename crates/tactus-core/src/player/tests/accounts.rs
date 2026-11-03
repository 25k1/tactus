use super::*;
use crate::accounts::AccountEntry;

fn signed_in(id: &str) -> Profile {
    let entry = AccountEntry { id: id.into(), ..AccountEntry::default() };
    let mut profile = Profile::new(entry, Arc::new(FakeProvider(fake().clone())));
    profile.service = Some(fake().clone());
    profile.account = Some(Account { id: "1".into(), ..Account::default() });
    profile
}

fn two_accounts() -> (Player, MockOutput) {
    let (mut player, output) = player_with(Settings::default());
    player.profile = signed_in("yandex-1");
    player.others.push(signed_in("yandex-2"));
    player.owner = "yandex-1".into();
    (player, output)
}

fn ids(queue: &Queue) -> Vec<String> {
    queue.ids()
}

#[test]
fn switching_keeps_playback_and_each_account_keeps_its_queue() {
    let (mut player, output) = two_accounts();
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    player.switch_account("yandex-2");
    assert!(!player.is_owner() && player.playing && output.0.borrow().playing);
    assert!(player.visible_queue().is_empty());
    player.add_to_queue(tracks(&["3"]));
    assert_eq!(ids(player.visible_queue()), ["3"]);
    assert_eq!(ids(&player.queue), ["1", "2"]);
    player.play_tracks(tracks(&["4", "5"]), 0, "test");
    settle(&mut player);
    assert!(player.is_owner());
    assert_eq!(player.track.as_ref().unwrap().id, "4");
    player.switch_account("yandex-1");
    assert_eq!(ids(player.visible_queue()), ["1", "2"]);
    player.play_index(1);
    settle(&mut player);
    assert_eq!(player.track.as_ref().unwrap().id, "2");
    player.switch_account("yandex-2");
    assert_eq!(ids(player.visible_queue()), ["4", "5"]);
}

#[test]
fn results_land_in_the_account_that_asked_for_them() {
    let (mut player, _) = two_accounts();
    player.load(&View::Album("70".into()), false);
    player.switch_account("yandex-2");
    settle(&mut player);
    assert!(player.page(&View::Album("70".into())).is_none());
    player.switch_account("yandex-1");
    assert!(matches!(player.page(&View::Album("70".into())), Some(PageState::Ready(_))));
}

#[test]
fn transport_follows_the_playing_account() {
    let (mut player, _) = two_accounts();
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    player.switch_account("yandex-2");
    player.next();
    settle(&mut player);
    assert_eq!(player.track.as_ref().unwrap().id, "2");
    assert_eq!(player.queue.index(), 1);
    assert!(!player.is_owner());
}

#[test]
fn signing_out_the_playing_account_hands_playback_to_the_next_one() {
    let (mut player, output) = two_accounts();
    player.play_tracks(tracks(&["1"]), 0, "test");
    settle(&mut player);
    player.sign_out();
    assert_eq!(player.profile.id(), "yandex-2");
    assert!(player.is_owner() && player.queue.is_empty() && !output.0.borrow().loaded);
    player.sign_out();
    assert!(!player.is_signed_in() && !player.profile.entry.is_saved());
}
