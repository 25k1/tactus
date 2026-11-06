use crate::queue::{self, Placement};
use crate::session::{self, Session, State};
use std::collections::HashSet;
use tactus_api::{Relation, Track, WaveBatch, WaveEvent};
use tactus_winamp::winamp::State as PlayState;
use tactus_winamp::{speech, text};

const REFILL_AT: usize = 2;
const RECENT: usize = 10;
const FINISH_MARGIN: f64 = 5.0;

#[derive(Clone, Debug, PartialEq)]
struct Playing {
    id: String,
    position: f64,
    duration: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wave {
    pub session: String,
    pub batch: String,
    order: Vec<String>,
    known: HashSet<String>,
    current: Option<Playing>,
    pub fetching: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub id: Option<String>,
    pub position: f64,
    pub duration: f64,
    pub upcoming: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Feedback(WaveEvent),
    Refill,
}

impl Wave {
    pub fn new(session: &str) -> Self {
        Self { session: session.to_owned(), ..Self::default() }
    }

    pub fn accept(&mut self, batch: WaveBatch, disliked: &HashSet<String>) -> Vec<Track> {
        if !batch.batch.is_empty() {
            self.batch = batch.batch;
        }
        let tracks: Vec<Track> = batch
            .tracks
            .into_iter()
            .filter(|track| !disliked.contains(&track.id) && !self.known.contains(&track.id))
            .collect();
        for track in &tracks {
            self.known.insert(track.id.clone());
            self.order.push(track.id.clone());
        }
        tracks
    }

    pub fn contains(&self, id: &str) -> bool {
        self.known.contains(id)
    }

    pub fn recent(&self) -> Vec<String> {
        self.order[self.order.len().saturating_sub(RECENT)..].to_vec()
    }

    pub fn position_of(&self, id: &str) -> f64 {
        self.current.as_ref().filter(|playing| playing.id == id).map_or(0.0, |playing| playing.position)
    }

    pub fn observe(&mut self, now: Observation) -> Vec<Step> {
        let mut steps = Vec::new();
        if self.current.as_ref().map(|playing| &playing.id) == now.id.as_ref() {
            if let Some(playing) = self.current.as_mut() {
                playing.position = now.position;
                playing.duration = now.duration;
            }
        } else {
            if let Some(previous) = self.current.take() {
                let track_id = previous.id;
                let seconds = previous.position;
                let finished = previous.duration > 0.0 && seconds >= previous.duration - FINISH_MARGIN;
                steps.push(Step::Feedback(if finished {
                    WaveEvent::TrackFinished { track_id, seconds }
                } else {
                    WaveEvent::Skip { track_id, seconds }
                }));
            }
            if let Some(id) = now.id.filter(|id| self.known.contains(id)) {
                steps.push(Step::Feedback(WaveEvent::TrackStarted { track_id: id.clone() }));
                self.current = Some(Playing { id, position: now.position, duration: now.duration });
            }
        }
        if self.current.is_some() && now.upcoming < REFILL_AT && !self.fetching {
            self.fetching = true;
            steps.push(Step::Refill);
        }
        steps
    }
}

fn feedback(session: &'static Session, wave: &Wave, event: WaveEvent) {
    let (wave, batch) = (wave.session.clone(), wave.batch.clone());
    session::run(move || session.service()?.wave_feedback(&wave, &batch, &event));
}

pub fn start(seeds: Vec<String>) {
    let Some(session) = session::get() else {
        return;
    };
    session::run(move || {
        let batch = session.service()?.start_wave(&seeds)?;
        let mut wave = Wave::new(&batch.session);
        let tracks = wave.accept(batch, &session.state().library.disliked_tracks);
        queue::put(session, &tracks, Placement::Replace(0))?;
        feedback(session, &wave, WaveEvent::RadioStarted);
        session.state().wave = Some(wave);
        speech::say(text::text("notice-wave-started"));
        Ok(())
    });
}

fn refill(session: &'static Session, wave: &Wave) {
    let (wave_session, recent) = (wave.session.clone(), wave.recent());
    session::run(move || {
        let result = session.service().and_then(|service| service.wave_tracks(&wave_session, &recent));
        let mut state = session.state();
        let State { library, wave, .. } = &mut *state;
        let Some(wave) = wave.as_mut().filter(|wave| wave.session == wave_session) else {
            return Ok(());
        };
        wave.fetching = false;
        let tracks = wave.accept(result?, &library.disliked_tracks);
        drop(state);
        queue::extend(session, &tracks)
    });
}

pub fn tick() {
    let Some(session) = session::get() else {
        return;
    };
    let winamp = session.winamp;
    let mut state = session.state();
    let Some(wave) = state.wave.as_mut() else {
        return;
    };
    let playing = winamp.state() != PlayState::Stopped;
    let observation = Observation {
        id: winamp.current_track_id().filter(|_| playing),
        position: winamp.position_ms().max(0) as f64 / 1000.0,
        duration: winamp.length_ms().max(0) as f64 / 1000.0,
        upcoming: winamp.len().saturating_sub(winamp.index() + 1),
    };
    for step in wave.observe(observation) {
        match step {
            Step::Feedback(event) => feedback(session, wave, event),
            Step::Refill => refill(session, wave),
        }
    }
}

pub fn relation_changed(session: &'static Session, id: &str, relation: Relation, on: bool) {
    let state = session.state();
    let Some(wave) = state.wave.as_ref().filter(|wave| wave.contains(id)) else {
        return;
    };
    let track_id = id.to_owned();
    let seconds = wave.position_of(id);
    let event = match (relation, on) {
        (Relation::Like, true) => WaveEvent::Like { track_id },
        (Relation::Like, false) => WaveEvent::Unlike { track_id },
        (Relation::Dislike, true) => WaveEvent::Dislike { track_id, seconds },
        (Relation::Dislike, false) => WaveEvent::Undislike { track_id },
    };
    feedback(session, wave, event);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(ids: &[&str]) -> WaveBatch {
        WaveBatch {
            session: "s".into(),
            batch: "b".into(),
            tracks: ids.iter().map(|id| Track { id: (*id).into(), ..Track::default() }).collect(),
        }
    }

    fn at(id: Option<&str>, position: f64, upcoming: usize) -> Observation {
        Observation { id: id.map(str::to_owned), position, duration: 200.0, upcoming }
    }

    #[test]
    fn accepts_only_new_tracks_that_are_not_disliked() {
        let mut wave = Wave::new("s");
        let disliked = HashSet::from(["2".to_owned()]);
        let ids = |tracks: Vec<Track>| tracks.into_iter().map(|track| track.id).collect::<Vec<_>>();
        assert_eq!(ids(wave.accept(batch(&["1", "2", "3"]), &disliked)), ["1", "3"]);
        assert_eq!(ids(wave.accept(batch(&["3", "4"]), &disliked)), ["4"]);
        assert_eq!(wave.recent(), ["1", "3", "4"]);
        assert_eq!(wave.batch, "b");
    }

    #[test]
    fn reports_starts_skips_and_finishes() {
        let mut wave = Wave::new("s");
        wave.accept(batch(&["1", "2", "3"]), &HashSet::new());
        let started = |id: &str| Step::Feedback(WaveEvent::TrackStarted { track_id: id.into() });
        assert_eq!(wave.observe(at(Some("1"), 0.0, 2)), [started("1")]);
        assert_eq!(wave.observe(at(Some("1"), 30.0, 2)), []);
        let skip = Step::Feedback(WaveEvent::Skip { track_id: "1".into(), seconds: 30.0 });
        assert_eq!(wave.observe(at(Some("2"), 0.0, 1)), [skip, started("2"), Step::Refill]);
        assert_eq!(wave.observe(at(Some("2"), 198.0, 1)), []);
        let finished = Step::Feedback(WaveEvent::TrackFinished { track_id: "2".into(), seconds: 198.0 });
        assert_eq!(wave.observe(at(Some("3"), 1.0, 0)), [finished, started("3")]);
        wave.fetching = false;
        assert_eq!(wave.observe(at(Some("3"), 2.0, 0)), [Step::Refill]);
    }

    #[test]
    fn ignores_tracks_from_outside_the_wave() {
        let mut wave = Wave::new("s");
        wave.accept(batch(&["1"]), &HashSet::new());
        assert_eq!(wave.observe(at(Some("9"), 0.0, 0)), []);
        assert_eq!(wave.observe(at(None, 0.0, 0)), []);
    }
}
