use super::*;

impl Player {
    pub fn page(&self, view: &View) -> Option<&PageState> {
        self.pages.get(view)
    }

    fn notice_new_episodes(&mut self) {
        let Some(PageState::Ready(page)) = self.pages.get(&View::Podcasts) else {
            return;
        };
        let episodes: Vec<Track> = page
            .sections
            .iter()
            .filter(|section| section.kind == SectionKind::NewEpisodes)
            .flat_map(Section::tracks)
            .cloned()
            .collect();
        let unseen = self.session.unseen_episodes(&episodes);
        if self.settings.episode_notifications && !unseen.is_empty() {
            self.emit(Event::NewEpisodes(unseen));
        }
    }

    pub(super) fn poll_episodes(&mut self) {
        let due = self.last_episode_check.is_none_or(|checked| checked.elapsed() >= EPISODE_CHECK_INTERVAL);
        if self.settings.episode_notifications && self.is_signed_in() && due {
            self.last_episode_check = Some(Instant::now());
            self.load(&View::Podcasts, true);
        }
    }

    pub fn is_played(&self, track: &Track) -> bool {
        track.episode && self.session.played.contains(&track.id)
    }

    pub fn set_played(&mut self, tracks: &[Track], played: bool) {
        let episodes: Vec<&Track> = tracks.iter().filter(|track| track.episode).collect();
        if episodes.is_empty() {
            return;
        }
        for track in &episodes {
            self.session.set_played(&track.id, played);
        }
        self.emit(Event::Library);
        self.notice(Notice::Played { count: episodes.len(), played });
    }

    pub fn set_hide_played(&mut self, hide: bool) {
        self.update_settings(|settings| settings.hide_played = hide);
        self.emit(Event::Library);
    }

    pub fn searches(&self) -> &[String] {
        &self.session.searches
    }

    pub fn clear_searches(&mut self) {
        self.session.searches.clear();
        self.emit(Event::Searches);
    }

    pub fn load(&mut self, view: &View, force: bool) {
        if let View::Search(query) = view {
            self.session.remember_search(query);
            self.emit(Event::Searches);
        }
        if !force && matches!(self.pages.get(view), Some(PageState::Ready(_) | PageState::Loading)) {
            return;
        }
        self.pages.insert(view.clone(), PageState::Loading);
        self.emit(Event::Page(view.clone()));
        let key = view.clone();
        let target = view.clone();
        self.spawn(
            move |service, uid| service.page(&key, uid),
            move |player, result| {
                let state = match result {
                    Ok(page) => PageState::Ready(page),
                    Err(error) => {
                        let failure = error.failure();
                        player.fail(error);
                        PageState::Failed(failure)
                    }
                };
                if let (View::Search(_), PageState::Ready(page)) = (&target, &state) {
                    let count = page.sections.iter().skip(1).map(|section| section.items.len()).sum();
                    player.notice(Notice::SearchResults(count));
                }
                player.pages.insert(target.clone(), state);
                if target == View::Podcasts {
                    player.notice_new_episodes();
                }
                player.emit(Event::Page(target));
            },
        );
    }

    pub fn has_more(&self, view: &View, section: usize) -> bool {
        matches!(self.pages.get(view), Some(PageState::Ready(page)) if page.sections.get(section).is_some_and(|section| section.more.is_some()))
    }

    pub fn load_more(&mut self, view: &View, section: usize) {
        let Some(PageState::Ready(page)) = self.pages.get(view) else {
            return;
        };
        let Some((kind, next)) = page.sections.get(section).and_then(|found| Some((found.kind, found.more?))) else {
            return;
        };
        if !self.loading_more.insert((view.clone(), section)) {
            return;
        }
        let (key, target) = (view.clone(), view.clone());
        self.spawn(
            move |service, _| service.more(&key, kind, next),
            move |player, result| {
                player.loading_more.remove(&(target.clone(), section));
                let paged = match result {
                    Ok(paged) => paged,
                    Err(error) => return player.fail(error),
                };
                let Some(PageState::Ready(page)) = player.pages.get_mut(&target) else {
                    return;
                };
                let Some(found) = page.sections.get_mut(section) else {
                    return;
                };
                let added = found.extend(paged);
                let exhausted = found.more.is_none();
                player.emit(Event::Page(target.clone()));
                match added {
                    0 if !exhausted => player.load_more(&target, section),
                    0 => {}
                    count => player.notice(Notice::MoreLoaded(count)),
                }
            },
        );
    }

    pub(super) fn reload(&mut self, view: &View) {
        if self.pages.contains_key(view) {
            self.load(view, true);
        }
    }

    pub(super) fn with_page(&mut self, view: View, then: impl FnOnce(&mut Player, Page) + Send + 'static) {
        if let Some(PageState::Ready(page)) = self.pages.get(&view) {
            let page = page.clone();
            return then(self, page);
        }
        let key = view.clone();
        self.request(
            move |service, uid| service.page(&key, uid),
            move |player, page| {
                player.pages.insert(view.clone(), PageState::Ready(page.clone()));
                player.emit(Event::Page(view));
                then(player, page);
            },
        );
    }

    pub fn suggest(&mut self, text: &str) {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let query = text.clone();
        self.spawn(
            move |service, _| service.suggestions(&query),
            move |player, result| {
                if let Ok(suggestions) = result {
                    player.emit(Event::Suggestions(text, suggestions));
                }
            },
        );
    }

    pub fn request_track_info(&mut self, track: &Track) {
        let id = track.id.clone();
        self.request(
            move |service, _| service.track_info(&id),
            |player, info| player.emit(Event::TrackInfo(Box::new(info))),
        );
    }
}
