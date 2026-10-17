use super::*;

impl Player {
    fn gain(&self, track: &Track) -> f32 {
        if self.settings.normalize { audio::normalization_gain(track.loudness) } else { 1.0 }
    }

    fn present(&mut self, track: &Track, playing: bool, at: u64) {
        self.generation += 1;
        if self.track.as_ref().is_none_or(|current| current.episode != track.episode) {
            self.audio.set_speed(self.settings.speed_for(track.episode));
            self.emit(Event::Modes);
        }
        self.track = Some(track.clone());
        self.playing = playing;
        self.position_ms = at;
        self.lyrics = None;
        self.lyric_line = None;
        for event in [Event::Track, Event::Playback, Event::Position, Event::Lyrics, Event::Library] {
            self.emit(event);
        }
        self.media(MediaCommand::Track(track.clone()));
        self.media_playback();
    }

    fn take_prefetched(&mut self, id: &str) -> Option<Loaded> {
        match self.prefetched.take() {
            Some((prefetched, loaded)) if prefetched == id => Some(loaded),
            _ => None,
        }
    }

    fn fetcher(&self, id: &str) -> impl FnOnce(&dyn MusicService, u64) -> Result<Loaded> + Send + 'static {
        let (local, quality, loader, id) =
            (self.offline.source(id), self.settings.stream_quality, self.loader.clone(), id.to_owned());
        move |service, _| {
            loader(&match local {
                Some(source) => source,
                None => service.audio_source(&id, quality)?,
            })
        }
    }

    pub(super) fn open_current(&mut self, autoplay: bool, at_ms: u64) {
        let Some(track) = self.queue.current().cloned() else {
            return;
        };
        self.audio.stop();
        let at = if at_ms == 0 { self.session.resume_position(&track) } else { at_ms };
        self.present(&track, autoplay, at);
        if let Some(loaded) = self.take_prefetched(&track.id) {
            return self.start_loaded(&track, loaded, Start::new(autoplay, Duration::from_millis(at), 1.0));
        }
        let generation = self.generation;
        self.spawn(self.fetcher(&track.id), move |player, result| {
            if player.generation != generation {
                return;
            }
            match result {
                Ok(loaded) => {
                    player.start_loaded(&track, loaded, Start::new(autoplay, Duration::from_millis(at), 1.0));
                }
                Err(error) => {
                    player.playing = false;
                    player.emit(Event::Playback);
                    player.fail(error);
                }
            }
        });
    }

    pub(super) fn start_loaded(&mut self, track: &Track, loaded: Loaded, start: Start) {
        let start = Start { playing: start.playing && self.playing, gain: self.gain(track), ..start };
        if let Err(error) = self.audio.start(loaded, start) {
            self.playing = false;
            self.emit(Event::Playback);
            return self.fail(error);
        }
        self.started = Instant::now();
        self.playing = start.playing;
        self.emit(Event::Playback);
        self.media_playback();
        if start.playing && self.wave.contains(&track.id) {
            let (session, batch, track_id) =
                (self.wave.session.clone(), self.wave.batch_of(&track.id), track.id.clone());
            self.quiet(move |service, _| {
                service.wave_feedback(&session, &batch, &tactus_api::WaveEvent::TrackStarted { track_id })
            });
        }
        self.load_lyrics(track);
        self.save_session();
    }

    fn crossfade_target(&self, position: u64) -> Option<usize> {
        let fade = u64::from(self.settings.crossfade) * 1000;
        let current = self.track.as_ref()?;
        let sleeping = self.sleep.is_some_and(|sleep| sleep.timer == SleepTimer::EndOfTrack);
        if fade == 0 || sleeping || current.episode || current.duration_ms == 0 || position + fade < current.duration_ms
        {
            return None;
        }
        let index = self.queue.following(self.settings.repeat, false).filter(|index| *index != self.queue.index())?;
        let next = self.queue.get(index)?;
        let ready = self.prefetched.as_ref().is_some_and(|(id, _)| *id == next.id);
        (ready && !next.episode).then_some(index)
    }

    fn crossfade(&mut self, index: usize) {
        self.finish_current(false);
        let Some(track) = self.queue.select(index).cloned() else {
            return;
        };
        let Some(loaded) = self.take_prefetched(&track.id) else {
            return self.open_current(true, 0);
        };
        self.emit(Event::Queue);
        self.present(&track, true, 0);
        let fade = Duration::from_secs(u64::from(self.settings.crossfade));
        self.start_loaded(&track, loaded, Start { fade, ..Start::new(true, Duration::ZERO, 1.0) });
        self.sync_queue_position();
    }

    pub fn preview_equalizer(&mut self, gains: &audio::Gains) {
        self.audio.set_equalizer(gains);
    }

    pub fn set_sleep(&mut self, timer: Option<SleepTimer>) {
        self.sleep = timer.map(|timer| Sleep::new(timer, Instant::now()));
        self.notice(Notice::Sleep(timer));
    }

    pub fn sleep_timer(&self) -> Option<SleepTimer> {
        self.sleep.map(|sleep| sleep.timer)
    }

    pub fn sleep_remaining(&self) -> Option<Duration> {
        self.sleep.and_then(|sleep| sleep.remaining(Instant::now()))
    }

    fn fall_asleep(&mut self) {
        self.sleep = None;
        self.set_playing(false);
        self.notice(Notice::SleepStopped);
    }

    pub(super) fn poll_sleep(&mut self) {
        let Some(sleep) = self.sleep.as_mut().filter(|_| self.playing) else {
            return;
        };
        match sleep.step(Instant::now()) {
            sleep::Step::Wait => {}
            sleep::Step::Warn => self.notice(Notice::SleepSoon),
            sleep::Step::FadeOut => self.audio.fade_out(sleep::FADE),
            sleep::Step::Stop => self.fall_asleep(),
        }
    }

    pub(super) fn load_lyrics(&mut self, track: &Track) {
        let (id, generation) = (track.id.clone(), self.generation);
        self.spawn(
            move |service, _| service.lyrics(&id),
            move |player, result| {
                if player.generation == generation {
                    player.lyrics = result.ok().flatten();
                    player.lyric_line = None;
                    player.emit(Event::Lyrics);
                }
            },
        );
    }

    pub(super) fn poll_playback(&mut self) {
        let position = self.audio.position().as_millis() as u64;
        if position / 1000 != self.position_ms / 1000 {
            self.emit(Event::Position);
        }
        self.position_ms = position;
        let line = self.lyrics.as_ref().filter(|lyrics| lyrics.synced()).and_then(|lyrics| lyrics.line_at(position));
        if line != self.lyric_line {
            self.lyric_line = line;
            if let Some(line) = line {
                self.emit(Event::LyricLine(line));
            }
        }
        if !self.playing {
            return;
        }
        if self.audio.finished() && self.started.elapsed() > FINISH_GUARD {
            return self.advance(false);
        }
        if let Some(index) = self.crossfade_target(position) {
            return self.crossfade(index);
        }
        if self.wave.is_active() && !self.wave.fetching && self.queue.remaining() <= 2 {
            self.fetch_wave(false);
        }
        self.prefetch(position);
    }

    pub(super) fn prefetch(&mut self, position: u64) {
        let duration = self.track.as_ref().map_or(0, |track| track.duration_ms);
        if duration == 0 || position + PREFETCH_MS < duration || self.prefetching.is_some() || self.prefetched.is_some()
        {
            return;
        }
        let repeat = match self.settings.repeat {
            Repeat::One => return,
            repeat => repeat,
        };
        let Some(next) = self.queue.following(repeat, true).and_then(|index| self.queue.get(index)).cloned() else {
            return;
        };
        let id = next.id.clone();
        self.prefetching = Some(id.clone());
        self.spawn(self.fetcher(&next.id), move |player, result| {
            player.prefetching = None;
            if let Ok(loaded) = result {
                player.prefetched = Some((id, loaded));
            }
        });
    }

    pub(super) fn handle_media(&mut self, event: MediaEvent) {
        match event {
            MediaEvent::Play => self.set_playing(true),
            MediaEvent::Pause => self.set_playing(false),
            MediaEvent::Toggle => self.toggle_play(),
            MediaEvent::Next => self.next(),
            MediaEvent::Previous => self.previous(),
            MediaEvent::Stop => self.stop(),
            MediaEvent::SeekBy(delta) => self.seek_by(delta),
            MediaEvent::SetPosition(position) => self.seek(position),
            MediaEvent::Raise => self.emit(Event::Raise),
            MediaEvent::Quit => self.emit(Event::Quit),
        }
    }

    pub fn set_playing(&mut self, playing: bool) {
        if self.track.is_none() {
            return;
        }
        self.playing = playing;
        if self.audio.is_loaded() {
            self.audio.set_playing(playing);
        } else if playing {
            return self.open_current(true, self.position_ms);
        }
        self.emit(Event::Playback);
        self.media_playback();
    }

    pub fn toggle_play(&mut self) {
        self.set_playing(!self.playing);
    }

    pub fn seek(&mut self, position_ms: u64) {
        let duration = self.track.as_ref().map_or(0, |track| track.duration_ms);
        let position = if duration > 0 { position_ms.min(duration) } else { position_ms };
        self.audio.seek(Duration::from_millis(position));
        self.position_ms = position;
        self.emit(Event::Position);
        self.media_playback();
    }

    pub fn seek_by(&mut self, delta_ms: i64) {
        self.seek(self.position_ms.saturating_add_signed(delta_ms));
    }

    pub fn set_volume(&mut self, volume: i16) {
        let volume = volume.clamp(0, 100) as u8;
        self.update_settings(|settings| settings.volume = volume);
    }

    pub fn set_ducked(&mut self, ducked: bool) {
        let volume = self.settings.volume;
        self.audio.set_volume(if ducked { volume / 4 } else { volume });
    }

    pub fn change_volume(&mut self, delta: i16) {
        self.set_volume(i16::from(self.settings.volume) + delta);
    }

    fn spoken(&self) -> bool {
        self.track.as_ref().is_some_and(|track| track.episode)
    }

    pub fn speed(&self) -> f32 {
        self.settings.speed_for(self.spoken())
    }

    pub fn set_speed(&mut self, speed: f32) {
        let speed = ((speed.clamp(0.5, 2.0) * 20.0).round()) / 20.0;
        let spoken = self.spoken();
        self.update_settings(|settings| *if spoken { &mut settings.spoken_speed } else { &mut settings.speed } = speed);
    }

    pub fn change_speed(&mut self, delta: f32) {
        self.set_speed(self.speed() + delta);
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.update_settings(|settings| settings.repeat = repeat);
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.update_settings(|settings| settings.shuffle = shuffle);
    }

    pub fn next(&mut self) {
        self.advance(true);
    }

    pub fn previous(&mut self) {
        if self.position_ms > 3_000 {
            return self.seek(0);
        }
        if let Some(index) = self.queue.preceding(self.settings.repeat) {
            self.finish_current(true);
            self.play_index(index);
        }
    }

    pub(super) fn finish_current(&mut self, skipped: bool) {
        let Some(track) = self.track.clone() else {
            return;
        };
        let seconds = self.audio.position().as_secs_f64();
        self.session.remember_position(&track, self.position_ms);
        if seconds >= 1.0 {
            let play = Play { track: track.clone(), played_seconds: seconds as u32, from: PLAY_FROM.to_owned() };
            self.quiet(move |service, uid| service.report_play(uid, &play));
        }
        if self.wave.contains(&track.id) {
            let (session, batch) = (self.wave.session.clone(), self.wave.batch_of(&track.id));
            let event = if skipped {
                tactus_api::WaveEvent::Skip { track_id: track.id, seconds }
            } else {
                tactus_api::WaveEvent::TrackFinished { track_id: track.id, seconds }
            };
            self.quiet(move |service, _| service.wave_feedback(&session, &batch, &event));
        }
    }

    pub(super) fn advance(&mut self, manual: bool) {
        self.finish_current(manual);
        let repeat = self.settings.repeat;
        let timer = self.sleep_timer();
        let following = self.queue.following(repeat, manual);
        match following {
            _ if !manual && timer == Some(SleepTimer::EndOfTrack) => self.fall_asleep(),
            None if timer == Some(SleepTimer::EndOfQueue) => self.fall_asleep(),
            Some(index) if !manual && repeat == Repeat::One && index == self.queue.index() => {
                let gain = self.track.as_ref().map_or(1.0, |track| self.gain(track));
                match self.audio.restart(Start::new(true, Duration::ZERO, gain)) {
                    Ok(()) => {
                        self.started = Instant::now();
                        self.seek(0);
                    }
                    Err(_) => self.open_current(true, 0),
                }
            }
            Some(index) => self.play_index(index),
            None if self.wave.is_active() || !self.wave.seeds.is_empty() => self.fetch_wave(true),
            None if self.settings.continue_with_wave => match self.track.as_ref() {
                Some(track) => self.continue_wave(vec![format!("track:{}", track.id)]),
                None => self.halt(),
            },
            None => self.halt(),
        }
    }
}
