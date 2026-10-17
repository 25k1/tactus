use super::*;

impl Ui {
    pub(super) fn move_selected(self: &Rc<Self>, action: ItemAction) {
        if let Some(row) = self.browser.list.selected()
            && self.actions_for(&[row]).contains(&Some(action))
        {
            self.run_action(action, row);
        }
    }

    pub(super) fn remove_selected(self: &Rc<Self>) {
        let Some(row) = self.browser.list.selected() else {
            return;
        };
        let rows = self.target_rows(row);
        let actions = self.actions_for(&rows);
        if let Some(action) = [ItemAction::RemoveFromQueue, ItemAction::RemoveFromPlaylist]
            .into_iter()
            .find(|action| actions.contains(&Some(*action)))
        {
            self.run_selection(action, &rows);
        }
    }

    pub(crate) fn own_playlists(&self) -> Vec<Playlist> {
        let player = self.player.borrow();
        player.playlists.iter().filter(|playlist| player.is_own(playlist)).take(PLAYLIST_LIMIT).cloned().collect()
    }

    pub(super) fn on_page_button(self: &Rc<Self>, button: PageButton) {
        let content = self.content.borrow().clone();
        let section = self.browser.section_index();
        let subject_target = content.subject.as_ref().and_then(Item::target);
        match button {
            PageButton::Play | PageButton::Shuffle => {
                let tracks = content.tracks(section);
                let shuffle = button == PageButton::Shuffle;
                self.act(|player| {
                    if shuffle {
                        player.set_shuffle(true);
                    }
                    player.play_tracks(tracks, 0, &content.title);
                });
            }
            PageButton::Radio if let Some(seed) = content.subject.as_ref().and_then(Item::seed) => {
                self.act(|player| player.start_wave(vec![seed]));
            }
            PageButton::Like | PageButton::Unlike if let Some(target) = subject_target => {
                self.act(|player| player.set_relation(target, Relation::Like, button == PageButton::Like));
                self.render(None, None);
            }
            PageButton::Radio | PageButton::Like | PageButton::Unlike => {}
            PageButton::Download => {
                if let (Some(Item::Album(_) | Item::Playlist(_)), Some(view)) = (&content.subject, content.view.clone())
                {
                    self.act(|player| player.download_view(view));
                } else {
                    let tracks = content.tracks(section);
                    self.act(|player| player.download(tracks, false));
                }
            }
            PageButton::Refresh => self.refresh(),
            PageButton::ClearQueue => self.act(Player::clear_queue),
            PageButton::SaveQueue => {
                if let Some(title) =
                    dialogs::ask_text(&self.frame, &fl!("playlist-new-prompt"), &PageButton::SaveQueue.label(), "")
                {
                    self.act(|player| player.save_queue_as_playlist(&title));
                }
            }
            PageButton::OpenFolder => {
                let folder = self.player.borrow().settings.download_folder();
                let _ = std::fs::create_dir_all(&folder);
                launch_default_application(&folder.to_string_lossy());
            }
            PageButton::ClearDownloads => self.act(Player::clear_finished_downloads),
            PageButton::ClearSearches => self.act(Player::clear_searches),
            PageButton::Upload if let Some(Item::Playlist(playlist)) = &content.subject => {
                let dialog = FileDialog::builder(&self.frame)
                    .with_message(&fl!("upload-title"))
                    .with_wildcard(&fl!("upload-wildcard"))
                    .with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist | FileDialogStyle::Multiple)
                    .build();
                if dialog.show_modal() == ID_OK {
                    let files: Vec<std::path::PathBuf> = dialog.get_paths().into_iter().map(Into::into).collect();
                    if !files.is_empty() {
                        self.act(|player| player.upload(playlist, files));
                    }
                }
            }
            PageButton::Upload => {}
            PageButton::HidePlayed | PageButton::ShowPlayed => {
                self.act(|player| player.set_hide_played(button == PageButton::HidePlayed));
                self.render(None, None);
            }
        }
    }

    pub(super) fn new_playlist(self: &Rc<Self>, tracks: Vec<Track>) {
        if let Some(title) = dialogs::ask_text(&self.frame, &fl!("playlist-new-prompt"), &fl!("menu-new-playlist"), "")
        {
            self.act(|player| player.create_playlist(&title, false, tracks));
        }
    }

    pub(super) fn choose_playlist(self: &Rc<Self>, tracks: Vec<Track>) {
        let playlists = self.own_playlists();
        if playlists.is_empty() {
            return self.new_playlist(tracks);
        }
        let mut names: Vec<String> = playlists.iter().map(|playlist| playlist.title.clone()).collect();
        names.push(fl!("menu-new-playlist").replace('&', ""));
        let title = match tracks.as_slice() {
            [track] => track.full_title(),
            _ => fl!("tracks-count", count = tracks.len()),
        };
        let prompt = fl!("playlist-choose-prompt", title = title);
        match dialogs::choose(&self.frame, &prompt, &fl!("menu-add-to-playlist"), &names) {
            Some(index) if index < playlists.len() => {
                let playlist = playlists[index].clone();
                self.act(|player| player.add_to_playlist(&playlist, tracks));
            }
            Some(_) => self.new_playlist(tracks),
            None => {}
        }
    }
}
