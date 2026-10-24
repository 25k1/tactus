use super::*;

impl Ui {
    pub(crate) fn remember(&self) {
        let (filter, sort) = self.browser.arrangement();
        let spot = Spot {
            section: self.browser.section_index(),
            row: self.browser.list.selected().unwrap_or(0),
            filter,
            sort,
        };
        self.history.borrow_mut().remember(spot);
    }

    pub(crate) fn navigate(self: &Rc<Self>, location: Location, focus: bool) {
        self.remember();
        let visit = self.history.borrow_mut().push(location).clone();
        self.show_visit(visit, focus);
    }

    pub(super) fn show_visit(self: &Rc<Self>, visit: Visit, focus: bool) {
        *self.location.borrow_mut() = visit.location.clone();
        if let Some(section) = visit.location.section()
            && self.sidebar.get_selection() != Some(section.index() as u32)
        {
            self.sidebar.set_selection(section.index() as u32, true);
        }
        if visit.location == Location::Section(Section::Lyrics) {
            self.book.set_selection(LYRICS_PAGE);
            self.lyrics.update(&self.player.borrow());
            if focus {
                self.lyrics.lines.set_focus();
            }
            return;
        }
        self.book.set_selection(BROWSER_PAGE);
        if let Some(view) = visit.location.view(&self.browser.search.get_value()) {
            self.act(|player| player.load(&view, false));
        }
        self.browser.set_arrangement(&visit.spot.filter, visit.spot.sort);
        self.restore.set(Some((visit.spot.section, visit.spot.row)));
        self.render(None, None);
        if focus {
            self.focus_content();
        }
    }

    pub(crate) fn render(self: &Rc<Self>, section: Option<usize>, row: Option<usize>) {
        let location = self.location.borrow().clone();
        let query = self.browser.search.get_value();
        let content = view_model::content(&self.player.borrow(), &location, &query);
        let restore = match content.status {
            Status::Ready => self.restore.take(),
            _ => None,
        };
        let section = restore.map(|(section, _)| section).or(section).unwrap_or_else(|| self.browser.section_index());
        let row = restore.map(|(_, row)| row).or(row);
        let (buttons, marked) = {
            let player = self.player.borrow();
            let marked = (location == Location::Section(Section::Queue)).then(|| player.queue.index());
            (view_model::buttons(&player, &location, &content, section), marked)
        };
        let mode = (location == Location::Section(Section::Search), location == Location::Section(Section::Wave));
        self.browser.render(&content, &buttons, section, row, marked, mode);
        *self.content.borrow_mut() = content;
    }

    pub(crate) fn current_view(&self) -> Option<View> {
        self.location.borrow().view(&self.browser.search.get_value())
    }

    pub(crate) fn focus_content(&self) {
        match *self.location.borrow() {
            Location::Section(Section::Search) => self.browser.search.set_focus(),
            Location::Section(Section::Lyrics) => self.lyrics.lines.set_focus(),
            _ => self.browser.list.list.set_focus(),
        }
    }

    pub(super) fn next_pane(&self) {
        if self.sidebar.has_focus() {
            self.focus_content();
        } else if self.bar.has_focus() {
            self.sidebar.set_focus();
        } else {
            self.bar.focus();
        }
    }

    pub(super) fn go(self: &Rc<Self>, forward: bool) {
        self.remember();
        let visit = {
            let mut history = self.history.borrow_mut();
            if forward { history.forward().cloned() } else { history.back().cloned() }
        };
        if let Some(visit) = visit {
            self.show_visit(visit, true);
        }
    }

    pub(crate) fn run_search(self: &Rc<Self>) {
        let query = self.browser.search.get_value().trim().to_owned();
        self.pending_suggestion.borrow_mut().take();
        if *self.location.borrow() != Location::Section(Section::Search) {
            self.navigate(Location::Section(Section::Search), false);
        }
        if query.is_empty() {
            return self.render(None, None);
        }
        self.focus_results.set(true);
        self.act(|player| player.load(&View::Search(query), true));
        self.render(Some(0), Some(0));
    }

    pub(super) fn refresh(self: &Rc<Self>) {
        match self.current_view() {
            Some(View::Search(_)) => self.run_search(),
            Some(view) => self.act(|player| player.load(&view, true)),
            None => self.render(None, None),
        }
    }

    pub(super) fn seek_to_lyric(self: &Rc<Self>) {
        let time = self.lyrics.lines.get_selection().and_then(|line| {
            let player = self.player.borrow();
            player.lyrics.as_ref()?.lines.get(line as usize)?.time_ms
        });
        if let Some(time) = time {
            self.act(|player| player.seek(time));
        }
    }

    pub(crate) fn lyric_text(&self, line: usize) -> Option<(String, String)> {
        let player = self.player.borrow();
        let lyric = player.lyrics.as_ref()?.lines.get(line)?;
        Some((lyric.text.clone(), lyric.time_ms.map(format_time).unwrap_or_default()))
    }
}
