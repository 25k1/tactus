use super::*;
use tactus_api::{StreamEvent, WaveEvent, WaveSettings, is_generated};

impl Player {
    pub fn my_wave(&self) -> Vec<String> {
        std::iter::once(MY_WAVE.to_owned()).chain(self.settings.wave_seeds.iter().cloned()).collect()
    }

    pub fn start_my_wave(&mut self) {
        self.start_wave(self.my_wave());
    }

    pub fn wave_options(&self) -> Option<&WaveSettings> {
        self.wave_options.as_ref()
    }

    pub fn set_wave_seeds(&mut self, seeds: Vec<String>) {
        let restart = self.wave.seeds.first().is_some_and(|seed| seed == MY_WAVE);
        self.update_settings(|settings| settings.wave_seeds = seeds);
        if restart {
            self.start_my_wave();
        }
    }

    pub(super) fn load_wave_options(&mut self) {
        self.spawn(
            |service| service.wave_settings(),
            |player, result| match result {
                Ok(options) => player.wave_options = Some(options),
                Err(error) => log::warn(format_args!("wave settings: {error}")),
            },
        );
    }

    pub fn play_last_wave(&mut self) {
        self.request(
            |service| service.last_wave(),
            |player, last| {
                let seeds = if last.seeds.is_empty() { player.my_wave() } else { last.seeds };
                player.start_wave(seeds);
            },
        );
    }

    pub fn reset_last_wave(&mut self) {
        self.request(|service| service.reset_last_wave(), |player, ()| player.notice(Notice::LastWaveReset));
    }

    pub(super) fn play_generated(&mut self, station: String) {
        self.request(
            move |service| service.generated_stream(&station),
            |player, stream| player.play_tracks(vec![stream.track()], 0, &stream.title),
        );
    }

    pub fn is_generated_playing(&self) -> bool {
        self.track.as_ref().is_some_and(|track| is_generated(&track.id))
    }

    pub(super) fn generated_feedback(&mut self, event: StreamEvent) {
        let Some(station) = self.track.as_ref().map(|track| track.id.clone()).filter(|id| is_generated(id)) else {
            return;
        };
        let stream = self.generated_stream.guard().clone();
        if !stream.is_empty() {
            self.quiet(move |service| service.generated_feedback(&station, &stream, event));
        }
    }

    pub(super) fn wave_relation_feedback(&mut self, changes: &[Change]) {
        if !self.is_owner() {
            return;
        }
        for (target, relation, on) in changes {
            let Target::Track(track) = target else {
                continue;
            };
            if !self.wave.contains(&track.id) {
                continue;
            }
            let current = self.track.as_ref().is_some_and(|current| current.id == track.id);
            let seconds = if current { self.audio.position().as_secs_f64() } else { 0.0 };
            let track_id = track.id.clone();
            let event = match (relation, on) {
                (Relation::Like, true) => WaveEvent::Like { track_id },
                (Relation::Like, false) => WaveEvent::Unlike { track_id },
                (Relation::Dislike, true) => WaveEvent::Dislike { track_id, seconds },
                (Relation::Dislike, false) => WaveEvent::Undislike { track_id },
            };
            let (session, batch) = (self.wave.session.clone(), self.wave.batch_of(&track.id));
            self.quiet(move |service| service.wave_feedback(&session, &batch, &event));
        }
    }
}
