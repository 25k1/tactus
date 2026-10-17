use super::*;

impl Player {
    pub fn play_tracks(&mut self, tracks: Vec<Track>, start: usize, context: &str) {
        if tracks.is_empty() {
            return;
        }
        self.queue.replace(tracks, start);
        if self.settings.shuffle {
            self.queue.set_shuffle(true, seed());
        }
        self.wave.clear();
        self.emit(Event::Queue);
        self.open_current(true, 0);
        self.sync_new_queue(context);
    }

    pub fn play_view(&mut self, view: View) {
        let context = format!("{view:?}");
        self.with_page(view, move |player, page| player.play_tracks(page.tracks(), 0, &context));
    }

    pub fn enqueue_view(&mut self, view: View) {
        self.with_page(view, |player, page| player.add_to_queue(page.tracks()));
    }

    pub fn play_ids(&mut self, ids: Vec<String>) {
        self.request(move |service, _| service.tracks(&ids), |player, tracks| player.play_tracks(tracks, 0, "various"));
    }

    pub fn play_index(&mut self, index: usize) {
        if self.queue.select(index).is_some() {
            self.emit(Event::Queue);
            self.open_current(true, 0);
            self.sync_queue_position();
        }
    }

    pub fn play_next(&mut self, tracks: Vec<Track>) {
        if self.queue.is_empty() {
            return self.play_tracks(tracks, 0, "various");
        }
        let count = tracks.len();
        self.queue.insert_next(tracks);
        self.emit(Event::Queue);
        self.notice(Notice::PlayNext(count));
    }

    pub fn add_to_queue(&mut self, tracks: Vec<Track>) {
        if self.queue.is_empty() {
            return self.play_tracks(tracks, 0, "various");
        }
        let count = tracks.len();
        self.queue.append(tracks);
        self.emit(Event::Queue);
        self.notice(Notice::Queued(count));
    }

    pub fn remove_from_queue(&mut self, indices: &[usize]) {
        let current = self.queue.index();
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        let mut removed: Vec<(usize, Track)> =
            indices.into_iter().rev().filter_map(|index| Some((index, self.queue.remove(index)?))).collect();
        if removed.is_empty() {
            return;
        }
        removed.reverse();
        let was_current = removed.iter().any(|(index, _)| *index == current);
        self.notice(Notice::RemovedFromQueue(removed.len()));
        self.remember_undo(Undo::Queue(removed));
        self.emit(Event::Queue);
        if was_current {
            if self.queue.is_empty() {
                self.halt();
            } else {
                self.open_current(self.playing, 0);
            }
        }
    }

    pub fn move_in_queue(&mut self, from: usize, to: usize) {
        if self.queue.move_item(from, to) {
            self.emit(Event::Queue);
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear_upcoming();
        self.wave.clear();
        self.emit(Event::Queue);
        self.notice(Notice::QueueCleared);
    }

    pub fn stop(&mut self) {
        self.set_playing(false);
        self.seek(0);
    }

    pub(super) fn halt(&mut self) {
        self.audio.stop();
        self.playing = false;
        self.emit(Event::Playback);
        self.media_playback();
    }

    pub fn start_wave(&mut self, seeds: Vec<String>) {
        self.begin_wave(seeds, true);
    }

    pub(super) fn begin_wave(&mut self, seeds: Vec<String>, autoplay: bool) {
        let request = seeds.clone();
        self.request(
            move |service, _| service.start_wave(&request),
            move |player, batch| {
                let (session, batch_id) = (batch.session.clone(), batch.batch.clone());
                player.wave.begin(seeds, session.clone());
                let tracks = player.wave.accept(batch, &player.library.disliked_tracks);
                player.queue.replace(tracks, 0);
                player.emit(Event::Queue);
                if autoplay {
                    player.notice(Notice::WaveStarted);
                }
                player.quiet(move |service, _| {
                    service.wave_feedback(&session, &batch_id, &tactus_api::WaveEvent::RadioStarted)
                });
                player.open_current(autoplay, 0);
            },
        );
    }

    pub(super) fn fetch_wave(&mut self, play: bool) {
        if !self.wave.is_active() {
            if play && !self.wave.seeds.is_empty() {
                self.continue_wave(self.wave.seeds.clone());
            }
            return;
        }
        self.wave.play_when_ready |= play;
        if self.wave.fetching {
            return;
        }
        self.wave.fetching = true;
        let session = self.wave.session.clone();
        let ids = self.queue.ids();
        let recent = ids[ids.len().saturating_sub(wave::QUEUE_LIMIT)..].to_vec();
        self.spawn(
            move |service, _| service.wave_tracks(&session, &recent),
            |player, result| {
                player.wave.fetching = false;
                let play = std::mem::take(&mut player.wave.play_when_ready);
                let tracks = match result {
                    Ok(batch) => player.wave.accept(batch, &player.library.disliked_tracks),
                    Err(error) => return player.fail(error),
                };
                if tracks.is_empty() {
                    return;
                }
                let first = player.queue.len();
                player.queue.append(tracks);
                player.emit(Event::Queue);
                if play {
                    player.play_index(first);
                }
            },
        );
    }

    pub(super) fn continue_wave(&mut self, seeds: Vec<String>) {
        let request = seeds.clone();
        self.request(
            move |service, _| service.start_wave(&request),
            move |player, batch| {
                player.wave.begin(seeds, batch.session.clone());
                let tracks = player.wave.accept(batch, &player.library.disliked_tracks);
                if tracks.is_empty() {
                    return;
                }
                let first = player.queue.len();
                player.queue.append(tracks);
                player.emit(Event::Queue);
                player.play_index(first);
            },
        );
    }

    pub(super) fn sync_new_queue(&mut self, context: &str) {
        self.remote_queue = None;
        if !self.settings.sync_queue || self.settings.ynison {
            return;
        }
        let (tracks, index, context) = (self.queue.tracks().to_vec(), self.queue.index(), context.to_owned());
        self.spawn(
            move |service, _| service.create_queue(&tracks, index, &context),
            |player, result| match result {
                Ok(id) => player.remote_queue = Some(id),
                Err(error) => log::warn(format_args!("queue sync: {error}")),
            },
        );
    }

    pub(super) fn sync_queue_position(&mut self) {
        if let (true, false, Some(id)) = (self.settings.sync_queue, self.settings.ynison, self.remote_queue.clone()) {
            let index = self.queue.index();
            self.quiet(move |service, _| service.update_queue_position(&id, index));
        }
    }
}
