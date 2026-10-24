use super::*;

impl Ui {
    pub(crate) fn execute(self: &Rc<Self>, command: Command) {
        let track = self.current_track();
        match command {
            Command::NewPlaylist => self.new_playlist(Vec::new()),
            Command::PromoCode => {
                if let Some(code) =
                    dialogs::ask_text(&self.frame, &fl!("promo-code-prompt"), &fl!("menu-promo-code"), "")
                {
                    self.act(|player| player.consume_promo_code(&code));
                }
            }
            Command::SignOut => {
                if dialogs::confirm(&self.frame, &fl!("sign-out-confirm"), &fl!("menu-sign-out")) {
                    self.act(Player::sign_out);
                }
            }
            Command::Settings => self.show_settings(),
            Command::Hide => self.hide_window(),
            Command::Quit => {
                self.quitting.set(true);
                self.frame.close(true);
            }
            Command::PlayPause => {
                self.act(Player::toggle_play);
                self.announce_feedback(Announcement::Playback);
            }
            Command::Stop => self.act(Player::stop),
            Command::Next => self.act(Player::next),
            Command::Previous => self.act(Player::previous),
            Command::SeekForward | Command::SeekBack => {
                let delta = if command == Command::SeekForward { SEEK_STEP_MS } else { -SEEK_STEP_MS };
                self.act(|player| player.seek_by(delta));
                self.announce_feedback(Announcement::Seek);
            }
            Command::VolumeUp | Command::VolumeDown => {
                let delta = if command == Command::VolumeUp { VOLUME_STEP } else { -VOLUME_STEP };
                self.act(|player| player.change_volume(delta));
                self.announce_feedback(Announcement::Volume);
            }
            Command::SpeedUp | Command::SpeedDown | Command::SpeedReset => {
                self.act(|player| match command {
                    Command::SpeedUp => player.change_speed(SPEED_STEP),
                    Command::SpeedDown => player.change_speed(-SPEED_STEP),
                    _ => player.set_speed(1.0),
                });
                self.announce_speed();
            }
            Command::Shuffle => {
                self.act(|player| player.set_shuffle(!player.settings.shuffle));
                self.announce_feedback(Announcement::Modes);
            }
            Command::Repeat => {
                self.act(|player| player.set_repeat(player.settings.repeat.cycle()));
                self.announce_feedback(Announcement::Modes);
            }
            Command::NowPlaying => {
                let text = view_model::now_playing_details(&self.player.borrow());
                dialogs::show_text(&self.frame, &fl!("menu-now-playing"), &text);
            }
            Command::SpeakNowPlaying => {
                let text = view_model::now_playing_details(&self.player.borrow()).replace('\n', ", ");
                self.speech.say(&text, true);
            }
            Command::Back => self.go(false),
            Command::Forward => self.go(true),
            Command::Refresh => self.refresh(),
            Command::NextPane => self.next_pane(),
            Command::FocusSearch => {
                self.navigate(Location::Section(Section::Search), true);
                self.browser.search.select_all();
            }
            Command::FocusFilter => {
                if *self.location.borrow() != Location::Section(Section::Lyrics) {
                    self.browser.filter.set_focus();
                    self.browser.filter.select_all();
                }
            }
            Command::OpenUser => {
                if let Some(login) = dialogs::ask_text(&self.frame, &fl!("user-prompt"), &fl!("menu-open-user"), "") {
                    self.navigate(Location::View(View::User(login)), true);
                }
            }
            Command::ToggleWindow => self.toggle_window(),
            Command::MoveUp => self.move_selected(ItemAction::MoveUp),
            Command::MoveDown => self.move_selected(ItemAction::MoveDown),
            Command::ShowWindow => self.raise_window(),
            Command::Shortcuts => {
                let settings = self.settings();
                let global: Vec<(GlobalAction, String)> = GlobalAction::ALL
                    .into_iter()
                    .filter(|_| settings.global_hotkeys)
                    .filter_map(|action| Some((action, action.key_in(&settings.hotkeys)?)))
                    .collect();
                dialogs::show_text(&self.frame, &fl!("menu-shortcuts"), &command::shortcuts(&self.keymap(), &global));
            }
            Command::OpenDataFolder => crate::platform::open(&tactus_core::settings::data_dir()),
            Command::OpenLog => {
                if let Some(path) = tactus_core::log::path() {
                    crate::platform::open(&path);
                }
            }
            Command::CheckUpdates => self.act(|player| player.check_updates(true)),
            Command::About => {
                let text = fl!(
                    "about-text",
                    version = tactus_core::update::CURRENT,
                    repository = tactus_core::update::REPOSITORY,
                    folder = tactus_core::settings::app_dir().display().to_string()
                );
                dialogs::show_text(&self.frame, &fl!("menu-about"), &text);
            }
            Command::Devices => self.show_devices(),
            Command::Listen => self.listen_once(),
            Command::VoiceHelp => self.show_voice_help(),
            Command::Undo => {
                if !crate::platform::edit::focused(Edit::Undo) {
                    self.act(Player::undo);
                }
            }
            Command::SelectAll => {
                if !crate::platform::edit::focused(Edit::SelectAll) && self.browser.list.list.has_focus() {
                    self.browser.list.select_all();
                    let count = self.browser.list.len();
                    self.announce(Announcement::Queue, &fl!("list-selected-all", count = count), true);
                }
            }
            Command::Find => {
                let (labels, ids): (Vec<String>, Vec<MenuId>) = command::searchable(&self.keymap()).into_iter().unzip();
                let title = fl!("menu-find-command");
                if let Some(index) = dialogs::pick(&self.frame, &title, &fl!("find-command-prompt"), labels) {
                    self.on_menu(ids[index].id());
                }
            }
            Command::TrackInfo
            | Command::Like
            | Command::Dislike
            | Command::AddToPlaylist
            | Command::Download
            | Command::Wave
            | Command::Similar
            | Command::GoToAlbum
            | Command::GoToArtist
            | Command::CopyLink => {
                let Some(track) = track else {
                    return self.announce(Announcement::Errors, &fl!("now-playing-nothing"), true);
                };
                let action = match command {
                    Command::TrackInfo => ItemAction::TrackInfo,
                    Command::Like => {
                        return self.act(|player| player.toggle_relation(Target::Track(track), Relation::Like));
                    }
                    Command::Dislike => ItemAction::Dislike,
                    Command::AddToPlaylist => return self.choose_playlist(vec![track]),
                    Command::Download => ItemAction::Download,
                    Command::Wave => ItemAction::Radio,
                    Command::Similar => ItemAction::Similar,
                    Command::GoToAlbum => ItemAction::GoToAlbum,
                    Command::GoToArtist => ItemAction::GoToArtist,
                    _ => ItemAction::CopyLink,
                };
                self.apply_action(action, &Entry::Item(Item::Track(track)), None);
            }
        }
    }

    pub(super) fn on_menu(self: &Rc<Self>, id: Id) {
        match MenuId::from_id(id) {
            Some(MenuId::Command(Command::NewPlaylist)) => self.new_playlist(self.context_tracks()),
            Some(MenuId::Command(command)) => self.execute(command),
            Some(MenuId::Section(section)) => self.navigate(Location::Section(section), true),
            Some(MenuId::Action(action)) => {
                let rows = self.context_rows.borrow().clone();
                self.run_selection(action, &rows);
            }
            Some(MenuId::Playlist(index)) if let Some(playlist) = self.playlist_menu.borrow().get(index).cloned() => {
                let mut tracks = self.context_tracks();
                if tracks.is_empty() {
                    tracks.extend(self.current_track());
                }
                if !tracks.is_empty() {
                    self.act(|player| player.add_to_playlist(&playlist, tracks));
                }
            }
            Some(MenuId::Sleep(choice)) => self.set_sleep(choice),
            Some(MenuId::Playlist(_)) | None => {}
        }
    }

    pub(super) fn set_sleep(self: &Rc<Self>, choice: SleepChoice) {
        let timer = match choice {
            SleepChoice::Off => None,
            SleepChoice::Timer(timer) => Some(timer),
            SleepChoice::Custom => {
                let Some(minutes) = dialogs::ask_text(&self.frame, &fl!("sleep-prompt"), &fl!("menu-sleep"), "20")
                    .and_then(|text| text.parse::<u32>().ok())
                    .filter(|minutes| (1..=24 * 60).contains(minutes))
                else {
                    return;
                };
                Some(SleepTimer::Minutes(minutes))
            }
        };
        self.act(|player| player.set_sleep(timer));
    }
}
