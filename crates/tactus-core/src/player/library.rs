use super::*;

impl Player {
    pub fn has(&self, target: &Target, relation: Relation) -> bool {
        self.library.has(target, relation)
    }

    pub fn set_relation(&mut self, target: Target, relation: Relation, on: bool) {
        self.set_relations(vec![target], relation, on);
    }

    pub fn set_relations(&mut self, targets: Vec<Target>, relation: Relation, on: bool) {
        let mut undo = Vec::new();
        let changes: Vec<Change> = targets
            .into_iter()
            .filter(|target| self.has(target, relation) != on)
            .map(|target| {
                undo.push((target.clone(), relation, !on));
                if on && self.has(&target, relation.opposite()) {
                    undo.push((target.clone(), relation.opposite(), true));
                }
                (target, relation, on)
            })
            .collect();
        if changes.is_empty() {
            return;
        }
        let skip = relation == Relation::Dislike
            && on
            && changes.iter().any(|(target, ..)| {
                matches!((target, &self.track), (Target::Track(track), Some(current)) if track.id == current.id)
            });
        self.relate(changes, Some(Undo::Relations(undo)), Notice::Relation { relation, on });
        if skip {
            self.next();
        }
    }

    pub(super) fn relate(&mut self, changes: Vec<Change>, undo: Option<Undo>, notice: Notice) {
        for (target, relation, on) in &changes {
            self.library.apply(target, *relation, *on);
        }
        self.emit(Event::Library);
        let request = changes.clone();
        self.spawn(
            move |service, uid| {
                Ok(request
                    .into_iter()
                    .filter_map(|change| {
                        service.set_relation(uid, &change.0, change.1, change.2).err().map(|error| (change, error))
                    })
                    .collect::<Vec<_>>())
            },
            move |player, result| {
                let (failed, error) = match result {
                    Ok(failed) => {
                        let mut first = None;
                        let failed: Vec<Change> = failed
                            .into_iter()
                            .map(|(change, error)| {
                                first.get_or_insert(error);
                                change
                            })
                            .collect();
                        (failed, first)
                    }
                    Err(error) => (changes, Some(error)),
                };
                for (target, relation, on) in &failed {
                    player.library.apply(target, *relation, !on);
                }
                match error {
                    None => {
                        player.notice(notice);
                        if let Some(undo) = undo {
                            player.remember_undo(undo);
                        }
                        player.reload(&View::Collection);
                    }
                    Some(error) => {
                        player.emit(Event::Library);
                        player.fail(error);
                    }
                }
            },
        );
    }

    pub fn is_pinned(&self, target: &Target) -> bool {
        self.library.is_pinned(target)
    }

    pub fn set_pin(&mut self, item: Item, on: bool) {
        let Some(target) = item.target() else {
            return;
        };
        self.library.set_pinned(&item, on);
        self.emit(Event::Library);
        self.spawn(
            move |service, _| service.set_pin(&target, on),
            move |player, result| match result {
                Ok(()) => {
                    player.notice(Notice::Pinned(on));
                    player.reload(&View::Collection);
                }
                Err(error) => {
                    player.library.set_pinned(&item, !on);
                    player.emit(Event::Library);
                    player.fail(error);
                }
            },
        );
    }

    pub fn toggle_relation(&mut self, target: Target, relation: Relation) {
        let on = !self.has(&target, relation);
        self.set_relation(target, relation, on);
    }

    pub(super) fn playlists_changed(&mut self, key: PlaylistKey) {
        self.refresh_playlists();
        self.reload(&View::Playlists);
        self.reload(&View::Playlist(key));
    }

    pub fn create_playlist(&mut self, title: &str, public: bool, tracks: Vec<Track>) {
        let title = title.trim().to_owned();
        if title.is_empty() {
            return;
        }
        self.request(
            move |service, uid| service.create_playlist(uid, &title, public),
            move |player, playlist| {
                player.notice(Notice::PlaylistCreated(playlist.title.clone()));
                player.playlists.insert(0, playlist.clone());
                player.emit(Event::Playlists);
                player.reload(&View::Playlists);
                if !tracks.is_empty() {
                    player.add_to_playlist(&playlist, tracks);
                }
            },
        );
    }

    pub fn add_to_playlist(&mut self, playlist: &Playlist, tracks: Vec<Track>) {
        let (title, count) = (playlist.title.clone(), tracks.len());
        let edit = PlaylistEdit::Insert { at: playlist.track_count, tracks };
        self.edit_playlist(playlist, edit, Notice::AddedToPlaylist { title, count });
    }

    pub fn change_playlist(&mut self, playlist: &Playlist, edit: PlaylistEdit) {
        let notice = match &edit {
            PlaylistEdit::Rename(_) => Notice::PlaylistRenamed,
            PlaylistEdit::Visibility(public) => Notice::PlaylistVisibility(*public),
            PlaylistEdit::Delete => Notice::PlaylistDeleted,
            PlaylistEdit::Insert { tracks, .. } => {
                Notice::AddedToPlaylist { title: playlist.title.clone(), count: tracks.len() }
            }
            PlaylistEdit::Remove(removed) => Notice::RemovedFromPlaylist(removed.len()),
            PlaylistEdit::Restore(_) => Notice::Undone,
            PlaylistEdit::Description(_) => Notice::PlaylistDescribed,
            PlaylistEdit::Move { .. } => Notice::MovedInPlaylist,
        };
        self.edit_playlist(playlist, edit, notice);
    }

    pub fn duplicate_playlist(&mut self, playlist: &Playlist, title: &str) {
        let title = title.to_owned();
        self.with_page(View::Playlist(playlist.key), move |player, page| {
            player.create_playlist(&title, false, page.tracks());
        });
    }

    pub(super) fn edit_playlist(&mut self, playlist: &Playlist, edit: PlaylistEdit, notice: Notice) {
        let key = playlist.key;
        let deleting = edit == PlaylistEdit::Delete;
        let undo = match &edit {
            PlaylistEdit::Remove(removed) => Some(Undo::Playlist(playlist.clone(), removed.clone())),
            _ => None,
        };
        self.request(
            move |service, uid| service.edit_playlist(uid, key.kind, &edit),
            move |player, ()| {
                player.notice(notice);
                if let Some(undo) = undo {
                    player.remember_undo(undo);
                }
                if deleting {
                    player.playlists.retain(|playlist| playlist.key != key);
                    player.pages.remove(&View::Playlist(key));
                }
                player.playlists_changed(key);
            },
        );
    }

    pub fn upload(&mut self, playlist: &Playlist, files: Vec<std::path::PathBuf>) {
        let key = playlist.key;
        self.spawn(
            move |service, uid| {
                let mut sent = 0;
                for file in &files {
                    let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                    service.upload(uid, key.kind, &name, std::fs::read(file)?)?;
                    sent += 1;
                }
                Ok(sent)
            },
            move |player, result| match result {
                Ok(sent) => {
                    player.notice(Notice::Uploaded(sent));
                    player.playlists_changed(key);
                }
                Err(error) => {
                    player.fail(error);
                    player.playlists_changed(key);
                }
            },
        );
    }

    pub fn save_queue_as_playlist(&mut self, title: &str) {
        let tracks = self.queue.tracks().to_vec();
        self.create_playlist(title, false, tracks);
    }

    pub fn download(&mut self, tracks: Vec<Track>, numbered: bool) {
        let Some(service) = self.service.clone() else {
            return self.fail(Error::SignedOut);
        };
        let folder = self.settings.download_folder();
        let count = tracks.len();
        for (index, track) in tracks.into_iter().enumerate() {
            let number = numbered.then_some(index + 1);
            let job = Job {
                path: download::render_path(&folder, &self.settings.download_template, &track, number),
                track,
                number,
                quality: self.settings.download_quality,
                cover: self.settings.embed_cover,
                service: service.clone(),
            };
            let download = self.downloader.enqueue(job);
            self.downloads.push(download);
        }
        self.emit(Event::Downloads);
        self.notice(Notice::DownloadQueued(count));
    }

    pub fn download_view(&mut self, view: View) {
        let numbered = matches!(view, View::Album(_) | View::Playlist(_));
        self.with_page(view, move |player, page| player.download(page.tracks(), numbered));
    }

    pub fn clear_finished_downloads(&mut self) {
        self.downloads.retain(|download| !download.is_done());
        self.emit(Event::Downloads);
    }

    pub(super) fn apply_download(&mut self, update: download::Update) {
        let Some(download) = self.downloads.iter_mut().find(|download| download.id == update.id) else {
            return;
        };
        download.state = update.state.clone();
        let track = download.track.clone();
        let title = track.full_title();
        self.emit(Event::Downloads);
        match update.state {
            DownloadState::Finished(path) => {
                self.offline.add(&track, &path);
                if self.persist
                    && let Err(error) = self.offline.save()
                {
                    log::warn(format_args!("downloads index: {error}"));
                }
                self.notice(Notice::Downloaded(title));
            }
            DownloadState::Failed(error) => self.notice(Notice::DownloadFailed { title, error }),
            _ => {}
        }
    }
}
