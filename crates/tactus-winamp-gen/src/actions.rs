use crate::session::{self, Session};
use crate::wave;
use tactus_api::error::Result;
use tactus_api::{Relation, Target};
use tactus_winamp::sdk::COMMAND_NEXT;
use tactus_winamp::text::text_with;
use tactus_winamp::winamp::State;
use tactus_winamp::{speech, text, tracks};

fn notice(relation: Relation, on: bool) -> &'static str {
    match (relation, on) {
        (Relation::Like, true) => "notice-liked",
        (Relation::Like, false) => "notice-unliked",
        (Relation::Dislike, true) => "notice-disliked",
        (Relation::Dislike, false) => "notice-undisliked",
    }
}

pub fn toggle(session: &'static Session, target: &Target, relation: Relation) -> Result<bool> {
    let on = !session.state().library.has(target, relation);
    session.service()?.set_relation(target, relation, on)?;
    session.state().library.apply(target, relation, on);
    if let Target::Track(track) = target {
        wave::relation_changed(session, &track.id, relation, on);
    }
    speech::say(text::text(notice(relation, on)));
    Ok(on)
}

fn with_current(action: impl FnOnce(&'static Session, String)) {
    let Some(session) = session::get() else {
        return;
    };
    match session.winamp.current_track_id() {
        Some(id) => action(session, id),
        None => speech::say(text::text("winamp-not-yandex")),
    }
}

fn toggle_current(relation: Relation) {
    with_current(|session, id| {
        session::run(move || {
            let target = Target::Track(session.connection.track(&id)?);
            let on = toggle(session, &target, relation)?;
            let skip = relation == Relation::Dislike && on;
            if skip && session.winamp.current_track_id().as_deref() == Some(id.as_str()) {
                session.winamp.command(COMMAND_NEXT);
            }
            Ok(())
        });
    });
}

pub fn like_current() {
    toggle_current(Relation::Like);
}

pub fn dislike_current() {
    toggle_current(Relation::Dislike);
}

pub fn wave_from_current() {
    with_current(|_, id| wave::start(vec![format!("track:{id}")]));
}

pub fn my_wave() {
    wave::start(vec![tracks::MY_WAVE.to_owned()]);
}

fn describe(state: State, title: &str, position_ms: i64, length_ms: i64) -> String {
    let time = |milliseconds: i64| tracks::duration(milliseconds.max(0) as u64);
    let elapsed = if length_ms > 0 {
        text_with("winamp-time-of", &[("position", &time(position_ms)), ("length", &time(length_ms))])
    } else {
        time(position_ms)
    };
    match state {
        State::Playing => text_with("winamp-playing", &[("title", title), ("time", &elapsed)]),
        State::Paused => text_with("winamp-paused", &[("title", title), ("time", &elapsed)]),
        State::Stopped => text_with("winamp-stopped", &[("title", title)]),
    }
}

pub fn say_now_playing() {
    let Some(session) = session::get() else {
        return;
    };
    let winamp = session.winamp;
    let message = match winamp.title(winamp.index()).filter(|_| !winamp.is_empty()) {
        Some(title) => describe(winamp.state(), &title, winamp.position_ms(), winamp.length_ms()),
        None => text::text("winamp-playlist-empty"),
    };
    speech::say(message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_the_current_track_and_position() {
        assert_eq!(describe(State::Playing, "Кино - Блюз", 83_000, 225_000), "Now playing: Кино - Блюз, 1:23 of 3:45");
        assert_eq!(describe(State::Paused, "Intro", 5_000, -1), "Paused: Intro, 0:05");
        assert_eq!(describe(State::Stopped, "Intro", -1, 60_000), "Stopped: Intro");
    }
}
