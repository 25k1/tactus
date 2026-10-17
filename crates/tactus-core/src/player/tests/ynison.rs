use super::*;
use crate::player::ynison::Remote;
use std::cell::RefCell;
use std::rc::Rc;
use tactus_api::ynison::{Device, Incoming, Outgoing, Playable, Playing, Shared};

#[derive(Clone, Default)]
struct FakeRemote {
    sent: Rc<RefCell<Vec<Outgoing>>>,
    incoming: Rc<RefCell<Vec<Incoming>>>,
}

impl Remote for FakeRemote {
    fn send(&self, outgoing: Outgoing) {
        self.sent.borrow_mut().push(outgoing);
    }

    fn take(&self) -> Vec<Incoming> {
        self.incoming.borrow_mut().drain(..).collect()
    }
}

fn shared(active: &str, queue: &[&str], index: usize, paused: bool) -> Shared {
    Shared {
        devices: ["pc", "phone"].map(|id| Device { id: id.into(), title: id.into(), ..Device::default() }).to_vec(),
        active: Some(active.into()),
        queue: queue.iter().map(|id| Playable { id: (*id).into(), album_id: "70".into() }).collect(),
        index: Some(index),
        playing: Playing { paused, speed: 1.0, ..Playing::default() },
    }
}

fn connected() -> (Player, MockOutput, FakeRemote) {
    let (mut player, output) = player_with(Settings { device_id: "pc".into(), ..Settings::default() });
    let remote = FakeRemote::default();
    player.attach_remote(Box::new(remote.clone()));
    (player, output, remote)
}

#[test]
fn announces_itself_and_takes_over_when_playing_locally() {
    let (mut player, _output, remote) = connected();
    assert!(matches!(remote.sent.borrow()[0], Outgoing::Full { active: false, .. }));
    remote.incoming.borrow_mut().push(Incoming::State(shared("phone", &["9"], 0, false)));
    player.poll();
    assert_eq!(player.active_device(), Some("phone"));
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    let sent = remote.sent.borrow();
    assert!(sent.contains(&Outgoing::Activate("pc".into())));
    assert!(
        sent.iter().any(|message| matches!(message, Outgoing::Full { active: true, queue, .. } if queue.len() == 2))
    );
}

#[test]
fn follows_remote_commands_while_active_and_pauses_when_handed_off() {
    let (mut player, _output, remote) = connected();
    player.play_tracks(tracks(&["1", "2"]), 0, "test");
    settle(&mut player);
    remote.incoming.borrow_mut().push(Incoming::State(shared("pc", &["1", "2"], 0, false)));
    player.poll();
    remote.incoming.borrow_mut().push(Incoming::State(shared("pc", &["1", "2"], 1, false)));
    player.poll();
    settle(&mut player);
    assert_eq!(player.queue.index(), 1);
    remote.incoming.borrow_mut().push(Incoming::State(shared("pc", &["1", "2"], 1, true)));
    player.poll();
    assert!(!player.playing);
    player.set_playing(true);
    remote.incoming.borrow_mut().push(Incoming::State(shared("phone", &["1", "2"], 1, false)));
    player.poll();
    assert!(!player.playing);
}

#[test]
fn controls_the_active_remote_device() {
    let (mut player, _output, remote) = connected();
    remote.incoming.borrow_mut().push(Incoming::State(shared("phone", &["7", "8"], 0, false)));
    player.poll();
    player.remote_skip(true);
    player.remote_toggle();
    player.move_to("phone");
    let sent = remote.sent.borrow();
    assert!(sent.iter().any(|message| matches!(message, Outgoing::Queue { index: Some(1), .. })));
    assert!(sent.iter().any(|message| matches!(message, Outgoing::Status(Playing { paused: true, .. }))));
    assert_eq!(sent.last(), Some(&Outgoing::Activate("phone".into())));
}
