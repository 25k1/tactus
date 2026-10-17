use super::*;

impl Ui {
    pub(super) fn context_tracks(&self) -> Vec<Track> {
        let rows = self.context_rows.borrow().clone();
        self.browser.list.entries(&rows).into_iter().filter_map(|(_, entry)| entry.track().cloned()).collect()
    }

    pub(super) fn target_rows(&self, row: usize) -> Vec<usize> {
        let selection = self.browser.list.selection();
        if selection.len() > 1 && selection.contains(&row) { selection } else { vec![row] }
    }

    pub(super) fn actions_for(&self, rows: &[usize]) -> Vec<Option<ItemAction>> {
        let Some(&first) = rows.first() else {
            return Vec::new();
        };
        let context = self.list_context(first);
        let player = self.player.borrow();
        if let [row] = rows {
            return self
                .browser
                .list
                .entry(*row)
                .map(|entry| view_model::item_actions(&player, &entry, &context))
                .unwrap_or_default();
        }
        let entries: Vec<Entry> = self.browser.list.entries(rows).into_iter().map(|(_, entry)| entry).collect();
        view_model::selection_actions(&player, &entries, &context)
    }

    pub(super) fn list_context(&self, row: usize) -> ListContext {
        let content = self.content.borrow();
        let player = self.player.borrow();
        ListContext {
            in_queue: *self.location.borrow() == Location::Section(Section::Queue),
            own_playlist: match &content.subject {
                Some(Item::Playlist(playlist)) if player.is_own(playlist) => Some(playlist.key),
                _ => None,
            },
            section: content.sections.get(self.browser.section_index()).map(|section| section.kind),
            index: self.browser.list.origin(row).unwrap_or(row),
            len: self.browser.list.len(),
            arranged: self.browser.arranged(),
        }
    }

    pub(super) fn activate(self: &Rc<Self>, row: usize) {
        if let Some(action) = self.browser.list.entry(row).as_ref().and_then(view_model::default_action) {
            self.run_action(action, row);
        }
    }

    pub(super) fn show_context_menu(self: &Rc<Self>, row: usize) {
        let rows = self.target_rows(row);
        let actions = self.actions_for(&rows);
        if actions.is_empty() {
            return;
        }
        let mut menu = Menu::builder().build();
        for action in actions {
            match action {
                None => {
                    menu.append_separator();
                }
                Some(ItemAction::AddToPlaylist) => {
                    let playlists = self.own_playlists();
                    let submenu = Menu::builder().build();
                    for (index, playlist) in playlists.iter().enumerate() {
                        let text = format!(
                            "{} ({})",
                            escape(&playlist.title),
                            fl!("tracks-count", count = playlist.track_count)
                        );
                        submenu.append(MenuId::Playlist(index).id(), &text, "", ItemKind::Normal);
                    }
                    if !playlists.is_empty() {
                        submenu.append_separator();
                    }
                    submenu.append(Command::NewPlaylist.id(), &fl!("menu-new-playlist"), "", ItemKind::Normal);
                    *self.playlist_menu.borrow_mut() = playlists;
                    menu.append_submenu(submenu, &ItemAction::AddToPlaylist.label(), "");
                }
                Some(action) => {
                    menu.append(MenuId::Action(action).id(), &action.label(), "", ItemKind::Normal);
                }
            }
        }
        *self.context_rows.borrow_mut() = rows;
        self.browser.list.list.popup_menu(&mut menu, None);
        self.context_rows.borrow_mut().clear();
    }

    pub(super) fn run_selection(self: &Rc<Self>, action: ItemAction, rows: &[usize]) {
        match rows {
            [] => {}
            [row] => self.run_action(action, *row),
            _ => self.apply_selection(action, rows),
        }
    }

    fn apply_selection(self: &Rc<Self>, action: ItemAction, rows: &[usize]) {
        let picked = self.browser.list.entries(rows);
        let tracks: Vec<Track> = picked.iter().filter_map(|(_, entry)| entry.track().cloned()).collect();
        let targets: Vec<Target> = picked.iter().filter_map(|(_, entry)| entry.item().and_then(Item::target)).collect();
        let in_queue = *self.location.borrow() == Location::Section(Section::Queue);
        let playlist = match &self.content.borrow().subject {
            Some(Item::Playlist(playlist)) => Some(playlist.clone()),
            _ => None,
        };
        match action {
            ItemAction::Play if in_queue && let Some((origin, _)) = picked.first() => {
                let index = *origin;
                self.act(|player| player.play_index(index));
            }
            ItemAction::Play => {
                let context = self.content.borrow().title.clone();
                self.act(|player| player.play_tracks(tracks, 0, &context));
            }
            ItemAction::PlayNext => self.act(|player| player.play_next(tracks)),
            ItemAction::AddToQueue => self.act(|player| player.add_to_queue(tracks)),
            ItemAction::AddToPlaylist => self.choose_playlist(tracks),
            ItemAction::Download => self.act(|player| player.download(tracks, false)),
            ItemAction::Like | ItemAction::Unlike | ItemAction::Dislike | ItemAction::Undislike => {
                let relation = match action {
                    ItemAction::Like | ItemAction::Unlike => Relation::Like,
                    _ => Relation::Dislike,
                };
                let on = matches!(action, ItemAction::Like | ItemAction::Dislike);
                self.act(|player| player.set_relations(targets, relation, on));
            }
            ItemAction::RemoveFromQueue => {
                let origins: Vec<usize> = picked.iter().map(|(origin, _)| *origin).collect();
                self.act(|player| player.remove_from_queue(&origins));
            }
            ItemAction::RemoveFromPlaylist if let Some(playlist) = playlist => {
                let removed: Vec<(usize, Track)> =
                    picked.iter().filter_map(|(origin, entry)| Some((*origin, entry.track()?.clone()))).collect();
                self.act(|player| player.change_playlist(&playlist, PlaylistEdit::Remove(removed)));
            }
            ItemAction::MarkPlayed | ItemAction::MarkUnplayed => {
                let played = action == ItemAction::MarkPlayed;
                self.act(|player| player.set_played(&tracks, played));
            }
            ItemAction::CopyLink => {
                let links: Vec<String> = picked
                    .iter()
                    .filter_map(|(_, entry)| {
                        entry.item().and_then(Item::link).or_else(|| entry.track().map(Track::link))
                    })
                    .collect();
                self.copy(&links.join("\r\n"));
            }
            _ => {}
        }
    }

    pub(super) fn run_action(self: &Rc<Self>, action: ItemAction, row: usize) {
        let Some(entry) = self.browser.list.entry(row) else {
            return;
        };
        self.apply_action(action, &entry, Some(row));
    }

    pub(super) fn apply_action(self: &Rc<Self>, action: ItemAction, entry: &Entry, row: Option<usize>) {
        let item = entry.item();
        let track = entry.track();
        let view = item.and_then(View::of);
        let in_queue = *self.location.borrow() == Location::Section(Section::Queue);
        let origin = row.and_then(|row| self.browser.list.origin(row));
        let playlist = match &self.content.borrow().subject {
            Some(Item::Playlist(playlist)) => Some(playlist.clone()),
            _ => None,
        };
        let navigate = |view: View| self.navigate(Location::View(view), true);
        match (action, item) {
            (ItemAction::Open, Some(Item::Link(link))) => {
                launch_default_browser(&link.url, BrowserLaunchFlags::Default);
            }
            (ItemAction::Open, _) if let Some(view) = &view => navigate(view.clone()),
            (ItemAction::Play, Some(Item::Track(_))) if let Some(row) = row => {
                if in_queue {
                    let index = origin.unwrap_or(row);
                    self.act(|player| player.play_index(index));
                } else {
                    let (tracks, start) = self.browser.list.tracks_from(row);
                    let context = self.content.borrow().title.clone();
                    self.act(|player| player.play_tracks(tracks, start, &context));
                }
            }
            (ItemAction::Play, None)
                if matches!(entry, Entry::Download(_))
                    && let Some(row) = row =>
            {
                let (tracks, start) = self.browser.list.tracks_from(row);
                self.act(|player| player.play_tracks(tracks, start, &fl!("nav-downloads")));
            }
            (ItemAction::Play, Some(Item::Station(station))) => {
                let seeds = wave::seeds(&station.seed);
                self.act(|player| player.start_wave(seeds));
            }
            (ItemAction::Play, _) if let Some(view) = &view => self.act(|player| player.play_view(view.clone())),
            (ItemAction::PlayNext, _) if let Some(track) = track => {
                let tracks = vec![track.clone()];
                self.act(|player| player.play_next(tracks));
            }
            (ItemAction::AddToQueue, _) if let Some(track) = track => {
                let tracks = vec![track.clone()];
                self.act(|player| player.add_to_queue(tracks));
            }
            (ItemAction::PlayNext | ItemAction::AddToQueue, _) if let Some(view) = &view => {
                self.act(|player| player.enqueue_view(view.clone()));
            }
            (ItemAction::Like | ItemAction::Unlike | ItemAction::Dislike | ItemAction::Undislike, Some(item))
                if let Some(target) = item.target() =>
            {
                let relation = match action {
                    ItemAction::Like | ItemAction::Unlike => Relation::Like,
                    _ => Relation::Dislike,
                };
                let on = matches!(action, ItemAction::Like | ItemAction::Dislike);
                self.act(|player| player.set_relation(target, relation, on));
            }
            (ItemAction::AddToPlaylist, _) if let Some(track) = track => self.choose_playlist(vec![track.clone()]),
            (ItemAction::AddToThisPlaylist, _) if let (Some(playlist), Some(track)) = (&playlist, track) => {
                let tracks = vec![track.clone()];
                self.act(|player| player.add_to_playlist(playlist, tracks));
            }
            (ItemAction::RemoveFromPlaylist, _)
                if let (Some(playlist), Some(track), Some(index)) = (&playlist, track, origin) =>
            {
                let edit = PlaylistEdit::Remove(vec![(index, track.clone())]);
                self.act(|player| player.change_playlist(playlist, edit));
            }
            (ItemAction::RemoveFromQueue, _) if let Some(index) = origin => {
                self.act(|player| player.remove_from_queue(&[index]));
            }
            (ItemAction::MoveUp | ItemAction::MoveDown, _)
                if let Some(row) = row
                    && let Some(target) = match action {
                        ItemAction::MoveUp => row.checked_sub(1),
                        _ => Some(row + 1),
                    }
                    && target < self.browser.list.len() =>
            {
                match (in_queue, &playlist, track) {
                    (true, _, _) => self.act(|player| player.move_in_queue(row, target)),
                    (false, Some(playlist), Some(track)) => {
                        let edit = PlaylistEdit::Move { from: row, to: target, track: track.clone() };
                        self.act(|player| player.change_playlist(playlist, edit));
                    }
                    _ => return,
                }
                self.browser.list.select(target);
            }
            (ItemAction::Download, _) if let Some(track) = track => {
                let tracks = vec![track.clone()];
                self.act(|player| player.download(tracks, false));
            }
            (ItemAction::Download, _) if let Some(view) = &view => {
                self.act(|player| player.download_view(view.clone()));
            }
            (ItemAction::Radio, Some(item)) if let Some(seed) = item.seed() => {
                self.act(|player| player.start_wave(vec![seed]));
            }
            (ItemAction::Similar, _) if let Some(track) = track => navigate(View::Track(track.id.clone())),
            (ItemAction::GoToAlbum, _)
                if let Some(track) = track
                    && !track.album_id.is_empty() =>
            {
                navigate(View::Album(track.album_id.clone()));
            }
            (ItemAction::GoToArtist, Some(Item::Album(album))) if !album.artist_id.is_empty() => {
                navigate(View::Artist(album.artist_id.clone()));
            }
            (ItemAction::GoToArtist, _)
                if let Some(track) = track
                    && !track.artist_id.is_empty() =>
            {
                navigate(View::Artist(track.artist_id.clone()));
            }
            (ItemAction::Pin | ItemAction::Unpin, Some(item)) => {
                let (item, on) = (item.clone(), action == ItemAction::Pin);
                self.act(|player| player.set_pin(item, on));
            }
            (ItemAction::MarkPlayed | ItemAction::MarkUnplayed, _) if let Some(track) = track => {
                let tracks = vec![track.clone()];
                self.act(|player| player.set_played(&tracks, action == ItemAction::MarkPlayed));
            }
            (ItemAction::TrackInfo, _) if let Some(track) = track => {
                self.act(|player| player.request_track_info(track));
            }
            (ItemAction::CopyLink, _)
                if let Some(link) = item.and_then(Item::link).or_else(|| track.map(Track::link)) =>
            {
                self.copy(&link);
            }
            (ItemAction::Rename, Some(Item::Playlist(playlist))) => {
                let prompt = fl!("playlist-rename-prompt");
                if let Some(title) = dialogs::ask_text(&self.frame, &prompt, &action.label(), &playlist.title) {
                    self.act(|player| player.change_playlist(playlist, PlaylistEdit::Rename(title)));
                }
            }
            (ItemAction::EditDescription, Some(Item::Playlist(playlist))) => {
                let prompt = fl!("playlist-description-prompt");
                if let Some(text) = dialogs::ask_text(&self.frame, &prompt, &action.label(), &playlist.description) {
                    self.act(|player| player.change_playlist(playlist, PlaylistEdit::Description(text)));
                }
            }
            (ItemAction::Duplicate, Some(Item::Playlist(playlist))) => {
                let suggested = fl!("playlist-copy-title", title = playlist.title.as_str());
                if let Some(title) =
                    dialogs::ask_text(&self.frame, &fl!("playlist-new-prompt"), &action.label(), &suggested)
                {
                    self.act(|player| player.duplicate_playlist(playlist, &title));
                }
            }
            (ItemAction::MakePublic | ItemAction::MakePrivate, Some(Item::Playlist(playlist))) => {
                let edit = PlaylistEdit::Visibility(action == ItemAction::MakePublic);
                self.act(|player| player.change_playlist(playlist, edit));
            }
            (ItemAction::Delete, Some(Item::Playlist(playlist))) => {
                let question = fl!("playlist-delete-confirm", title = playlist.title.clone());
                if dialogs::confirm(&self.frame, &question, &action.label()) {
                    self.act(|player| player.change_playlist(playlist, PlaylistEdit::Delete));
                }
            }
            (ItemAction::OpenFile, _)
                if let Entry::Download(download) = entry
                    && let Some(path) = download.file() =>
            {
                launch_default_application(&path.to_string_lossy());
            }
            (ItemAction::ShowFile, _)
                if let Entry::Download(download) = entry
                    && let Some(path) = download.file() =>
            {
                crate::platform::show_in_folder(path);
            }
            _ => {}
        }
    }
}
