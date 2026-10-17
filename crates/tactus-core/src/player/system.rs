use super::*;

impl Player {
    pub fn check_updates(&mut self, manual: bool) {
        self.background(update::latest, move |player, result| match result {
            Ok(Some(release)) => player.emit(Event::UpdateAvailable(Box::new(release))),
            Ok(None) if manual => player.notice(Notice::UpToDate),
            Ok(None) => {}
            Err(error) if manual => player.fail(error),
            Err(error) => log::warn(format_args!("update check: {error}")),
        });
    }

    pub fn install_update(&mut self, release: update::Release) {
        self.save_session();
        self.background(
            move || update::install(&release),
            |player, result| match result {
                Ok(path) => player.emit(Event::Updated(path)),
                Err(error) => player.fail(error),
            },
        );
    }

    pub fn attach_window(&mut self, window: usize) {
        let media = Media::start(window, self.settings.media_controls);
        if let Some(track) = &self.track {
            media.send(MediaCommand::Track(track.clone()));
        }
        self.media = Some(media);
    }

    pub(super) fn media(&self, command: MediaCommand) {
        if let Some(media) = &self.media {
            media.send(command);
        }
    }

    pub(super) fn media_playback(&self) {
        self.media(MediaCommand::Playback { playing: self.playing, position_ms: self.position_ms });
    }

    pub fn update_settings(&mut self, update: impl FnOnce(&mut Settings)) {
        let old = self.settings.clone();
        update(&mut self.settings);
        let new = self.settings.clone();
        if old.volume != new.volume {
            self.audio.set_volume(new.volume);
            self.emit(Event::Volume);
        }
        let spoken = self.track.as_ref().is_some_and(|track| track.episode);
        if old.speed_for(spoken) != new.speed_for(spoken) {
            self.audio.set_speed(new.speed_for(spoken));
            self.emit(Event::Modes);
        }
        if old.active_equalizer() != new.active_equalizer() {
            self.audio.set_equalizer(&new.active_equalizer());
        }
        if old.output_device != new.output_device
            && let Err(error) = self.audio.set_device(&new.output_device)
        {
            self.fail(error);
        }
        if (old.ynison, &old.device_name) != (new.ynison, &new.device_name) {
            self.connect_ynison();
        }
        if old.media_controls != new.media_controls {
            self.media(MediaCommand::Enabled(new.media_controls));
        }
        if old.shuffle != new.shuffle {
            self.queue.set_shuffle(new.shuffle, seed());
            self.emit(Event::Queue);
        }
        if old.repeat != new.repeat || old.shuffle != new.shuffle {
            self.emit(Event::Modes);
        }
        self.settings_dirty |= old != new;
    }

    pub(super) fn flush_settings(&mut self) {
        if !self.settings_dirty || !self.persist {
            return;
        }
        self.settings_dirty = false;
        if let Err(error) = self.settings.save() {
            self.fail(error);
        }
    }
}
