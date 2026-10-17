use super::*;

impl Player {
    pub fn begin_login(&mut self) {
        self.login = LoginState::Requesting;
        self.emit(Event::Login);
        self.login_generation += 1;
        let (generation, sender) = (self.login_generation, self.sender.clone());
        let set = move |state: LoginState| -> Update {
            Box::new(move |player: &mut Player| {
                if player.login_generation == generation {
                    player.login = state;
                    player.emit(Event::Login);
                }
            })
        };
        net::spawn_blocking(move || {
            let send = |update: Update| {
                let _ = sender.send(update);
            };
            let device = match auth::request_device_code() {
                Ok(device) => device,
                Err(error) => return send(set(LoginState::Failed(error.failure()))),
            };
            send(set(LoginState::Waiting { code: device.user_code.clone(), url: device.verification_url.clone() }));
            let deadline = Instant::now() + Duration::from_secs(device.expires_in.max(60));
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_secs(device.interval.max(2)));
                match auth::poll_device_token(&device.device_code) {
                    Ok(token) => {
                        return send(Box::new(move |player: &mut Player| {
                            if player.login_generation == generation {
                                player.finish_login(&token);
                            }
                        }));
                    }
                    Err(Error::AuthorizationPending) => {}
                    Err(error) => return send(set(LoginState::Failed(error.failure()))),
                }
            }
            send(set(LoginState::Failed(Error::Expired.failure())));
        });
    }

    pub(super) fn finish_login(&mut self, token: &str) {
        self.login = LoginState::Idle;
        if let Err(error) = settings::save_token(token) {
            self.login = LoginState::Failed(error.failure());
        }
        self.emit(Event::Login);
        self.connect_with(token);
    }

    pub(super) fn connect_with(&mut self, token: &str) {
        match (self.connect)(token) {
            Ok(service) => {
                self.service = Some(service);
                self.emit(Event::Login);
                self.initialize();
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn sign_out(&mut self) {
        self.audio.stop();
        if let Err(error) = settings::delete_token() {
            self.fail(error);
        }
        self.service = None;
        self.account = None;
        self.library = Library::default();
        self.playlists.clear();
        self.pages.clear();
        self.queue.clear();
        self.track = None;
        self.playing = false;
        self.lyrics = None;
        self.wave.clear();
        self.remote_queue = None;
        self.generation += 1;
        self.session = Session::default();
        if self.persist {
            let _ = self.session.save();
        }
        for event in [
            Event::Login,
            Event::Account,
            Event::Track,
            Event::Playback,
            Event::Queue,
            Event::Library,
            Event::Playlists,
        ] {
            self.emit(event);
        }
    }

    pub(super) fn poll_connection(&mut self) {
        let due = self.last_connect.is_some_and(|attempt| attempt.elapsed() >= RECONNECT_INTERVAL);
        if self.disconnected && self.account.is_none() && self.service.is_some() && due {
            self.initialize();
        }
    }

    pub(super) fn initialize(&mut self) {
        self.last_connect = Some(Instant::now());
        self.spawn(
            |service, _| service.account(),
            |player, result| {
                let account = match result {
                    Ok(account) => account,
                    Err(error) if error.kind() == ErrorKind::Network => {
                        if !player.disconnected {
                            player.disconnected = true;
                            player.notice(Notice::Offline);
                            player.emit(Event::Account);
                        }
                        return;
                    }
                    Err(error) => return player.fail(error),
                };
                if std::mem::take(&mut player.disconnected) {
                    player.notice(Notice::Online);
                    let failed: Vec<View> = player
                        .pages
                        .iter()
                        .filter(|(_, state)| matches!(state, PageState::Failed(_)))
                        .map(|(view, _)| view.clone())
                        .collect();
                    for view in failed {
                        player.load(&view, true);
                    }
                }
                player.account = Some(account);
                player.emit(Event::Account);
                player.refresh_library();
                player.refresh_playlists();
                player.spawn(
                    |service, _| service.account_settings(),
                    |player, result| {
                        if let Ok(settings) = result {
                            player.account_settings = settings;
                            player.emit(Event::AccountSettings);
                        }
                    },
                );
                player.restore();
                if player.settings.ynison && !player.has_remote() {
                    player.connect_ynison();
                }
            },
        );
    }

    pub(super) fn restore(&mut self) {
        let autoplay = self.settings.autoplay;
        if self.settings.resume_session && !self.session.tracks.is_empty() {
            let session = self.session.clone();
            self.queue.replace(session.tracks, session.index);
            self.wave.seeds = session.wave;
            self.emit(Event::Queue);
            self.open_current(autoplay, session.position_ms);
        } else if self.settings.sync_queue {
            self.spawn(
                |service, _| {
                    let Some(queue) = service.active_queue()? else {
                        return Ok(None);
                    };
                    Ok(Some((service.tracks(&queue.track_ids)?, queue)))
                },
                move |player, result| match result {
                    Ok(Some((tracks, queue))) if !tracks.is_empty() => {
                        player.queue.replace(tracks, queue.index);
                        player.remote_queue = Some(queue.id);
                        player.emit(Event::Queue);
                        player.open_current(autoplay, 0);
                    }
                    _ => player.begin_wave(vec![MY_WAVE.to_owned()], autoplay),
                },
            );
        } else {
            self.begin_wave(vec![MY_WAVE.to_owned()], autoplay);
        }
    }

    pub fn refresh_library(&mut self) {
        self.spawn(
            |service, uid| service.library(uid),
            |player, result| match result {
                Ok(library) => {
                    player.library = library;
                    player.emit(Event::Library);
                }
                Err(error) => log::warn(format_args!("library: {error}")),
            },
        );
    }

    pub fn refresh_playlists(&mut self) {
        self.spawn(
            |service, uid| service.playlists(uid),
            |player, result| {
                if let Ok(playlists) = result {
                    player.playlists = playlists;
                    player.emit(Event::Playlists);
                }
            },
        );
    }

    pub fn is_own(&self, playlist: &Playlist) -> bool {
        self.uid() != 0 && playlist.key.owner == self.uid()
    }

    pub fn set_account_option(&mut self, option: AccountOption, enabled: bool) {
        self.request(
            move |service, _| service.set_account_option(option, enabled),
            move |player, ()| {
                match player.account_settings.iter_mut().find(|(candidate, _)| *candidate == option) {
                    Some(entry) => entry.1 = enabled,
                    None => player.account_settings.push((option, enabled)),
                }
                player.emit(Event::AccountSettings);
                player.notice(Notice::AccountUpdated);
            },
        );
    }

    pub fn consume_promo_code(&mut self, code: &str) {
        let code = code.trim().to_owned();
        if code.is_empty() {
            return;
        }
        self.request(
            move |service, _| service.consume_promo_code(&code),
            |player, status| {
                player.notice(Notice::Promo(status));
                player.request(
                    |service, _| service.account(),
                    |player, account| {
                        player.account = Some(account);
                        player.emit(Event::Account);
                    },
                );
            },
        );
    }
}
