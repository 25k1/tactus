use super::*;

impl Player {
    pub fn begin_login(&mut self, kind: ProviderKind) {
        let Some(provider) = self.provider_for(kind) else {
            return self.fail(Error::Unsupported);
        };
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
            let mut login = match provider.login() {
                Ok(login) => login,
                Err(error) => return send(set(LoginState::Failed(error.failure()))),
            };
            let prompt = login.prompt().clone();
            send(set(LoginState::Waiting { code: prompt.code, url: prompt.url }));
            let deadline = Instant::now() + prompt.expires;
            while Instant::now() < deadline {
                std::thread::sleep(prompt.interval);
                match login.poll() {
                    Ok(Some(token)) => {
                        return send(Box::new(move |player: &mut Player| {
                            if player.login_generation == generation {
                                player.finish_login(kind, &token);
                            }
                        }));
                    }
                    Ok(None) => {}
                    Err(error) => return send(set(LoginState::Failed(error.failure()))),
                }
            }
            send(set(LoginState::Failed(Error::Expired.failure())));
        });
    }

    pub fn cancel_login(&mut self) {
        self.login_generation += 1;
        self.login = LoginState::Idle;
        self.emit(Event::Login);
    }

    pub fn destination(&self, link: &str) -> Option<tactus_api::Destination> {
        std::iter::once(&self.profile)
            .chain(&self.others)
            .find_map(|profile| profile.provider.destination(link))
            .or_else(|| self.providers.iter().find_map(|provider| provider.destination(link)))
    }

    pub fn share_link(&self, item: Option<&Item>, track: Option<&Track>) -> Option<String> {
        let provider = &self.profile.provider;
        match (item, track) {
            (Some(item), _) => provider.share_link(item),
            (None, Some(track)) => provider.share_link(&Item::Track(track.clone())),
            (None, None) => None,
        }
    }

    pub fn capabilities(&self) -> Capabilities {
        self.profile.capabilities()
    }

    pub fn accounts(&self) -> Vec<&Profile> {
        let mut profiles: Vec<&Profile> =
            std::iter::once(&self.profile).chain(&self.others).filter(|profile| profile.entry.is_saved()).collect();
        profiles.sort_by(|left, right| left.id().cmp(right.id()));
        profiles
    }

    fn entries(&self) -> Vec<AccountEntry> {
        self.accounts().into_iter().map(|profile| profile.entry.clone()).collect()
    }

    pub(super) fn save_accounts(&self) {
        if !self.persist {
            return;
        }
        let list = AccountList {
            accounts: self.entries(),
            active: self.profile.id().to_owned(),
            queue_owner: self.owner.clone(),
        };
        if let Err(error) = list.save() {
            log::warn(format_args!("accounts: {error}"));
        }
    }

    fn account_changed(&mut self) {
        for event in [Event::Login, Event::Account, Event::Library, Event::Playlists, Event::Queue, Event::Searches] {
            self.emit(event);
        }
    }

    pub fn switch_account(&mut self, id: &str) {
        let Some(index) = self.others.iter().position(|profile| profile.id() == id) else {
            return;
        };
        std::mem::swap(&mut self.profile, &mut self.others[index]);
        self.save_accounts();
        self.account_changed();
        self.notice(Notice::AccountSwitched(self.profile.label()));
    }

    pub fn rename_account(&mut self, alias: &str) {
        alias.trim().clone_into(&mut self.profile.entry.alias);
        self.save_accounts();
        self.emit(Event::Account);
    }

    pub fn set_account_overrides(
        &mut self,
        stream_quality: Option<tactus_api::Quality>,
        download_quality: Option<tactus_api::Quality>,
        proxy: Option<tactus_api::proxy::Proxy>,
    ) {
        let reconnect = self.profile.entry.proxy != proxy;
        let entry = &mut self.profile.entry;
        (entry.stream_quality, entry.download_quality, entry.proxy) = (stream_quality, download_quality, proxy);
        self.save_accounts();
        if reconnect {
            self.connect_saved();
        }
    }

    pub(super) fn stream_quality(&self) -> tactus_api::Quality {
        self.owner_profile().entry.stream_quality.unwrap_or(self.settings.stream_quality)
    }

    pub(super) fn download_quality(&self) -> tactus_api::Quality {
        self.profile.entry.download_quality.unwrap_or(self.settings.download_quality)
    }

    pub(super) fn finish_login(&mut self, kind: ProviderKind, token: &str) {
        let Some(provider) = self.provider_for(kind) else {
            return;
        };
        self.login = LoginState::Idle;
        let entry = AccountEntry::new(kind, &self.entries());
        if let Err(error) = entry.save_token(token) {
            self.login = LoginState::Failed(error.failure());
            return self.emit(Event::Login);
        }
        let previous = std::mem::replace(&mut self.profile, Profile::new(entry, provider));
        if previous.entry.is_saved() {
            self.others.push(previous);
        }
        if !self.has_account(&self.owner) {
            self.owner = self.profile.id().to_owned();
        }
        self.save_accounts();
        self.account_changed();
        self.connect_with(token);
    }

    pub(super) fn connect_saved(&mut self) {
        if let Some(token) = self.profile.entry.load_token() {
            self.connect_with(&token);
        }
    }

    pub(super) fn connect_with(&mut self, token: &str) {
        match self.profile.provider.connect(token, self.profile.entry.proxy.as_ref()) {
            Ok(service) => {
                self.profile.service = Some(service);
                self.emit(Event::Login);
                self.initialize();
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn sign_out(&mut self) {
        if !self.profile.entry.is_saved() {
            return;
        }
        if let Err(error) = self.profile.entry.forget() {
            self.fail(error);
        }
        let leaving_owner = self.is_owner();
        let next = match self.others.pop() {
            Some(next) => next,
            None => Profile::new(AccountEntry::default(), self.profile.provider.clone()),
        };
        self.profile = next;
        if leaving_owner {
            self.audio.stop();
            self.queue.clear();
            self.track = None;
            self.playing = false;
            self.lyrics = None;
            self.wave.clear();
            self.remote_queue = None;
            self.generation += 1;
            self.owner = self.profile.id().to_owned();
            self.adopt_shelf();
            self.emit(Event::Track);
            self.playback_changed();
        }
        self.save_accounts();
        self.account_changed();
    }

    pub(super) fn poll_connection(&mut self) {
        let due = self.profile.last_connect.is_some_and(|attempt| attempt.elapsed() >= RECONNECT_INTERVAL);
        if self.profile.disconnected && self.profile.account.is_none() && self.profile.service.is_some() && due {
            self.initialize();
        }
    }

    pub(super) fn initialize(&mut self) {
        self.profile.last_connect = Some(Instant::now());
        self.spawn(
            |service| service.account(),
            |player, result| {
                let account = match result {
                    Ok(account) => account,
                    Err(error) if error.kind() == ErrorKind::Network => {
                        if !player.profile.disconnected {
                            player.profile.disconnected = true;
                            player.notice(Notice::Offline);
                            player.emit(Event::Account);
                        }
                        return;
                    }
                    Err(error) => return player.fail(error),
                };
                if std::mem::take(&mut player.profile.disconnected) {
                    player.notice(Notice::Online);
                    let failed: Vec<View> = player
                        .profile
                        .pages
                        .iter()
                        .filter(|(_, state)| matches!(state, PageState::Failed(_)))
                        .map(|(view, _)| view.clone())
                        .collect();
                    for view in failed {
                        player.load(&view, true);
                    }
                }
                player.profile.account = Some(account);
                player.emit(Event::Account);
                player.refresh_library();
                player.refresh_playlists();
                player.spawn(
                    |service| service.account_settings(),
                    |player, result| {
                        if let Ok(settings) = result {
                            player.profile.account_settings = settings;
                            player.emit(Event::AccountSettings);
                        }
                    },
                );
                if player.profile.capabilities().contains(Capabilities::WAVE) {
                    player.load_wave_options();
                }
                if player.is_owner() {
                    player.restore();
                    if player.settings.ynison && !player.has_remote() {
                        player.connect_ynison();
                    }
                }
            },
        );
    }

    pub(super) fn restore(&mut self) {
        let restart = self.restart.take();
        let autoplay = restart.unwrap_or(self.settings.autoplay);
        if (self.settings.resume_session || restart.is_some()) && !self.profile.session.tracks.is_empty() {
            let session = self.profile.session.clone();
            self.queue.replace(session.tracks, session.index);
            self.wave.seeds = session.wave;
            self.emit(Event::Queue);
            self.open_current(autoplay, session.position_ms);
        } else if self.settings.sync_queue {
            self.spawn(
                |service| {
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
                    _ => player.begin_wave(player.my_wave(), autoplay),
                },
            );
        } else {
            self.begin_wave(self.my_wave(), autoplay);
        }
    }

    pub fn refresh_library(&mut self) {
        self.spawn(
            |service| service.library(),
            |player, result| match result {
                Ok(library) => {
                    player.profile.library = library;
                    player.emit(Event::Library);
                }
                Err(error) => log::warn(format_args!("library: {error}")),
            },
        );
    }

    pub fn refresh_playlists(&mut self) {
        self.spawn(
            |service| service.playlists(),
            |player, result| {
                if let Ok(playlists) = result {
                    player.profile.playlists = playlists;
                    player.emit(Event::Playlists);
                }
            },
        );
    }

    pub fn is_own(&self, playlist: &Playlist) -> bool {
        self.profile.account.as_ref().is_some_and(|account| !account.id.is_empty() && playlist.key.owner == account.id)
    }

    pub fn set_account_option(&mut self, option: AccountOption, enabled: bool) {
        self.request(
            move |service| service.set_account_option(option, enabled),
            move |player, ()| {
                match player.profile.account_settings.iter_mut().find(|(candidate, _)| *candidate == option) {
                    Some(entry) => entry.1 = enabled,
                    None => player.profile.account_settings.push((option, enabled)),
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
            move |service| service.consume_promo_code(&code),
            |player, status| {
                player.notice(Notice::Promo(status));
                player.request(
                    |service| service.account(),
                    |player, account| {
                        player.profile.account = Some(account);
                        player.emit(Event::Account);
                    },
                );
            },
        );
    }
}
