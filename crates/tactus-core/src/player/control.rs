use super::*;

impl Player {
    pub(super) fn adopt_shelf(&mut self) {
        self.queue = std::mem::take(&mut self.profile.shelf);
        self.wave = Wave::default();
        self.wave.seeds.clone_from(&self.profile.session.wave);
        self.emit(Event::Queue);
    }

    pub(super) fn claim_queue(&mut self) {
        if self.is_owner() {
            return;
        }
        let previous = self.owner.clone();
        self.within(&previous, |player| {
            player.save_session();
            player.profile.shelf = std::mem::take(&mut player.queue);
        });
        self.owner = self.profile.id().to_owned();
        self.audio.stop();
        self.generation += 1;
        self.prefetched = None;
        self.stream_retries = None;
        self.remote_queue = None;
        self.adopt_shelf();
        self.save_accounts();
    }

    pub fn visible_queue(&self) -> &Queue {
        if self.is_owner() { &self.queue } else { &self.profile.shelf }
    }

    fn edit_shelf(&mut self, edit: impl FnOnce(&mut Queue)) -> bool {
        if self.is_owner() {
            return false;
        }
        edit(&mut self.profile.shelf);
        self.profile.session.tracks = self.profile.shelf.tracks().to_vec();
        self.profile.session.index = self.profile.shelf.index();
        if self.persist
            && let Err(error) = self.profile.session.save(&self.profile.entry)
        {
            log::warn(format_args!("session: {error}"));
        }
        self.emit(Event::Queue);
        true
    }

    pub fn play_tracks(&mut self, tracks: Vec<Track>, start: usize, context: &str) {
        if tracks.is_empty() {
            return;
        }
        self.claim_queue();
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
        self.request(move |service| service.tracks(&ids), |player, tracks| player.play_tracks(tracks, 0, "various"));
    }

    pub fn play_index(&mut self, index: usize) {
        self.claim_queue();
        if self.queue.select(index).is_some() {
            self.emit(Event::Queue);
            self.open_current(true, 0);
            self.sync_queue_position();
        }
    }

    pub fn play_next(&mut self, tracks: Vec<Track>) {
        let count = tracks.len();
        if self.edit_shelf(|shelf| shelf.insert_next(tracks.clone())) {
            return self.notice(Notice::PlayNext(count));
        }
        if self.queue.is_empty() {
            return self.play_tracks(tracks, 0, "various");
        }
        self.queue.insert_next(tracks);
        self.emit(Event::Queue);
        self.notice(Notice::PlayNext(count));
    }

    pub fn add_to_queue(&mut self, tracks: Vec<Track>) {
        let count = tracks.len();
        if self.edit_shelf(|shelf| shelf.append(tracks.clone())) {
            return self.notice(Notice::Queued(count));
        }
        if self.queue.is_empty() {
            return self.play_tracks(tracks, 0, "various");
        }
        self.queue.append(tracks);
        self.emit(Event::Queue);
        self.notice(Notice::Queued(count));
    }

    pub fn remove_from_queue(&mut self, indices: &[usize]) {
        let removed = indices.len();
        if self.edit_shelf(|shelf| {
            let mut indices = indices.to_vec();
            indices.sort_unstable_by(|left, right| right.cmp(left));
            indices.dedup();
            for index in indices {
                shelf.remove(index);
            }
        }) {
            return self.notice(Notice::RemovedFromQueue(removed));
        }
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
        if self.edit_shelf(|shelf| {
            shelf.move_item(from, to);
        }) {
            return;
        }
        if self.queue.move_item(from, to) {
            self.emit(Event::Queue);
        }
    }

    pub fn clear_queue(&mut self) {
        if self.edit_shelf(Queue::clear) {
            return self.notice(Notice::QueueCleared);
        }
        self.queue.clear_upcoming();
        self.wave.clear();
        self.emit(Event::Queue);
        self.notice(Notice::QueueCleared);
    }

    pub fn stop(&mut self) {
        self.as_owner(|player| {
            player.set_playing(false);
            player.seek(0);
        });
    }

    pub(super) fn halt(&mut self) {
        self.audio.stop();
        self.stream_retries = None;
        self.playing = false;
        self.playback_changed();
    }

    pub fn start_wave(&mut self, seeds: Vec<String>) {
        if let [seed] = seeds.as_slice()
            && tactus_api::is_generated(seed)
        {
            return self.play_generated(seed.clone());
        }
        self.begin_wave(seeds, true);
    }

    pub(super) fn begin_wave(&mut self, seeds: Vec<String>, autoplay: bool) {
        self.claim_queue();
        let request = seeds.clone();
        self.request(
            move |service| service.start_wave(&request),
            move |player, batch| {
                let (session, batch_id) = (batch.session.clone(), batch.batch.clone());
                player.wave.begin(seeds, session.clone());
                let tracks = player.wave.accept(batch, &player.profile.library.disliked_tracks);
                player.queue.replace(tracks, 0);
                player.emit(Event::Queue);
                if autoplay {
                    player.notice(Notice::WaveStarted);
                }
                player.quiet(move |service| {
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
            move |service| service.wave_tracks(&session, &recent),
            |player, result| {
                player.wave.fetching = false;
                let play = std::mem::take(&mut player.wave.play_when_ready);
                let tracks = match result {
                    Ok(batch) => player.wave.accept(batch, &player.profile.library.disliked_tracks),
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
            move |service| service.start_wave(&request),
            move |player, batch| {
                player.wave.begin(seeds, batch.session.clone());
                let tracks = player.wave.accept(batch, &player.profile.library.disliked_tracks);
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
        if !self.settings.sync_queue || self.settings.ynison || self.is_generated_playing() {
            return;
        }
        let (tracks, index, context) = (self.queue.tracks().to_vec(), self.queue.index(), context.to_owned());
        self.spawn(
            move |service| service.create_queue(&tracks, index, &context),
            |player, result| match result {
                Ok(id) => player.remote_queue = Some(id),
                Err(error) => log::warn(format_args!("queue sync: {error}")),
            },
        );
    }

    pub(super) fn sync_queue_position(&mut self) {
        if let (true, false, Some(id)) = (self.settings.sync_queue, self.settings.ynison, self.remote_queue.clone()) {
            let index = self.queue.index();
            self.quiet(move |service| service.update_queue_position(&id, index));
        }
    }
}
