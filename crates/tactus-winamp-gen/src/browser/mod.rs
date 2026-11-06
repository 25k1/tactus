mod form;
mod pages;
mod window;

pub use window::{close, deliver, show};

pub fn account_changed() {
    deliver(Browser::account_changed);
}

use crate::actions;
use crate::queue::{self, Placement};
use crate::session::Session;
use crate::wave;
use form::*;
use pages::{Navigation, PLACES};
use tactus_api::error::{Error, Result};
use tactus_api::{Destination, Item, Relation, Target, View, net};
use tactus_winamp::speech;
use tactus_winamp::text::{self, text, text_with};
use tactus_winamp::ui::controls;
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, IsWindowEnabled};

type Menu = (HWND, Vec<(u16, String)>, (i32, i32));

pub struct Browser {
    session: &'static Session,
    form: Form,
    navigation: Navigation,
    focus_list: bool,
}

impl Browser {
    fn new(window: HWND, session: &'static Session) -> Self {
        let places: Vec<String> = PLACES.iter().map(|(key, _)| text(key)).collect();
        Self { session, form: Form::new(window, &places), navigation: Navigation::default(), focus_list: false }
    }

    fn start(&mut self) {
        self.form.layout();
        self.render();
        if self.session.connection.signed_in() {
            self.show_signed_in();
        } else {
            self.show_signed_out();
        }
    }

    fn show_signed_in(&mut self) {
        self.show_account();
        self.session.refresh(|| deliver(Browser::show_account));
        self.open_selected_place();
        controls::focus(self.form.places.control);
    }

    fn account_changed(&mut self) {
        if self.session.connection.signed_in() {
            return self.show_signed_in();
        }
        self.navigation.clear();
        self.render();
        self.show_signed_out();
    }

    fn show_signed_out(&mut self) {
        self.set_status(&text("winamp-browser-signed-out"), false);
        controls::focus(self.form.settings);
    }

    fn background<T: Send + 'static>(
        &self,
        work: impl FnOnce(&'static Session) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Browser, Result<T>) + Send + 'static,
    ) {
        let session = self.session;
        net::spawn_blocking(move || {
            let result = work(session);
            deliver(move |browser| done(browser, result));
        });
    }

    fn report<T>(&self, result: Result<T>) -> Option<T> {
        result.map_err(|error| self.fail(&error)).ok()
    }

    fn set_status(&self, message: &str, speak: bool) {
        controls::set_text(self.form.status.control, message);
        if speak {
            speech::say(message);
        }
    }

    fn fail(&self, error: &Error) {
        tactus_winamp::log::write(error);
        self.set_status(&text::error(error), true);
    }

    fn show_account(&mut self) {
        if let Some(name) = self.session.account_name() {
            self.set_status(&text_with("status-signed-in", &[("name", &name)]), false);
        }
    }

    fn focus_search(&mut self) {
        controls::focus(self.form.search.control);
    }

    fn render(&mut self) {
        let place = self.navigation.current();
        let names = place.and_then(|place| place.page.as_ref()).map(pages::section_names).unwrap_or_default();
        controls::fill_combo(self.form.section.control, &names, place.map_or(0, |place| place.section));
        let title = match place {
            Some(place) => format!("{} — {}", place.title, text("winamp-title")),
            None => text("winamp-title"),
        };
        controls::set_text(self.form.window, &title);
        self.fill_items();
    }

    fn fill_items(&mut self) {
        let rows =
            self.navigation.section().map(|section| section.items.iter().map(pages::row).collect()).unwrap_or_default();
        let selected = self.navigation.current().map_or(0, |place| place.item);
        controls::fill_list(self.form.list.control, rows, selected);
        self.update_buttons();
    }

    fn update_buttons(&self) {
        let item = self.navigation.item();
        let target = item.and_then(Item::target);
        let has = |relation| target.as_ref().is_some_and(|target| self.session.state().library.has(target, relation));
        let station = matches!(item, Some(Item::Station(_)));
        let playable = item.is_some_and(|item| item.track().is_some() || station || View::of(item).is_some());
        let states = [
            (PLAY, playable),
            (ENQUEUE, playable && !station),
            (NEXT, playable && !station),
            (OPEN, item.is_some() && !station),
            (RADIO, item.and_then(Item::seed).is_some()),
            (LIKE, target.as_ref().is_some_and(|target| !matches!(target, Target::Wave(_)))),
            (DISLIKE, matches!(item, Some(Item::Track(_) | Item::Artist(_)))),
            (MORE, self.navigation.section().is_some_and(|section| section.more.is_some())),
            (BACK, self.navigation.can_go_back()),
            (WAVE, self.session.connection.signed_in()),
        ];
        for (id, enabled) in states {
            controls::enable(self.form.button(id), enabled);
        }
        let labels = [
            (LIKE, has(Relation::Like), "winamp-unlike-item", "winamp-like-item"),
            (DISLIKE, has(Relation::Dislike), "winamp-undislike", "winamp-dislike"),
        ];
        for (id, on, undo, apply) in labels {
            controls::set_text(self.form.button(id), &text(if on { undo } else { apply }));
        }
    }

    fn open(&mut self, view: View, title: String, root: bool) {
        if root {
            self.navigation.root(view.clone(), title.clone());
        } else {
            self.navigation.push(view.clone(), title.clone());
        }
        self.focus_list = !root || unsafe { GetFocus() } == self.form.list.control;
        self.render();
        self.set_status(&text_with("page-loading", &[("title", &title)]), false);
        let requested = view.clone();
        self.background(
            move |session| session.service()?.page(&requested),
            move |browser, result| browser.loaded(&view, result),
        );
    }

    fn loaded(&mut self, view: &View, result: Result<tactus_api::Page>) {
        if self.navigation.current().is_none_or(|place| place.view != *view) {
            return;
        }
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                let title = self.navigation.title();
                let message = text_with("page-failed", &[("title", &title), ("error", &text::error(&error))]);
                return self.set_status(&message, true);
            }
        };
        self.navigation.loaded(view, page);
        self.render();
        if self.navigation.section().is_none() {
            self.set_status(&text_with("page-empty", &[("title", &self.navigation.title())]), true);
        } else {
            self.show_account();
        }
        if std::mem::take(&mut self.focus_list) {
            controls::focus(self.form.list.control);
        }
    }

    fn open_selected_place(&mut self) {
        if !self.session.connection.signed_in() {
            return;
        }
        let index = controls::combo_selection(self.form.places.control).unwrap_or(0).min(PLACES.len() - 1);
        let (key, view) = &PLACES[index];
        self.open(view.clone(), text(key), true);
    }

    fn refresh(&mut self) {
        if let Some(place) = self.navigation.current().cloned() {
            self.navigation.back();
            let root = self.navigation.current().is_none_or(|current| current.view == place.view);
            self.open(place.view, place.title, root);
        }
    }

    fn section_changed(&mut self) {
        if let Some(index) = controls::combo_selection(self.form.section.control) {
            self.navigation.select_section(index);
            self.fill_items();
        }
    }

    fn item_changed(&mut self, index: usize) {
        self.navigation.select_item(index);
        self.update_buttons();
    }

    fn enter(&mut self) {
        let focus = unsafe { GetFocus() };
        if focus == self.form.search.control {
            self.search();
        } else if focus == self.form.list.control {
            self.activate();
        } else if focus == self.form.places.control {
            window::cancel_place_timer(self.form.window);
            self.open_selected_place();
        } else {
            window::click(self.form.window, focus);
        }
    }

    fn search(&mut self) {
        let query = controls::text(self.form.search.control).trim().to_owned();
        if query.is_empty() || !self.session.connection.signed_in() {
            return;
        }
        match tactus_yandex::destination(&query) {
            Some(Destination::View(view)) => self.open(view, query, true),
            Some(Destination::Playlist(link) | Destination::Join { playlist: link, .. }) => self.background(
                move |session| session.service()?.find_playlist(&link),
                move |browser, result| {
                    if let Some(key) = browser.report(result) {
                        browser.open(View::Playlist(key), query, true);
                    }
                },
            ),
            None => self.open(View::Search(query.clone()), query, true),
        }
    }

    fn activate(&mut self) {
        match self.navigation.item() {
            Some(Item::Track(_) | Item::Station(_)) => self.put(Placement::Replace(0)),
            _ => self.open_item(),
        }
    }

    fn button(&mut self, id: u16) {
        match id {
            SEARCH => self.search(),
            PLAY => self.put(Placement::Replace(0)),
            ENQUEUE => self.put(Placement::Append),
            NEXT => self.put(Placement::Next),
            OPEN => self.open_item(),
            RADIO => self.radio(),
            LIKE => self.relation(Relation::Like),
            DISLIKE => self.relation(Relation::Dislike),
            MORE => self.more(),
            BACK => self.back(),
            WAVE => actions::my_wave(),
            _ => {}
        }
    }

    fn menu(&self, control: HWND, position: LPARAM) -> Option<Menu> {
        let list = self.form.list.control;
        if control != list || self.navigation.item().is_none() {
            return None;
        }
        let point = if position == -1 {
            controls::row_point(list, self.navigation.current().map_or(0, |place| place.item))
        } else {
            ((position & 0xffff) as i16 as i32, ((position >> 16) & 0xffff) as i16 as i32)
        };
        let items = ITEM_ACTIONS
            .iter()
            .map(|(id, _)| (*id, self.form.button(*id)))
            .filter(|(_, button)| unsafe { IsWindowEnabled(*button) } != 0)
            .map(|(id, button)| (id, text::plain(&controls::text(button))))
            .collect();
        Some((self.form.window, items, point))
    }

    fn put(&mut self, placement: Placement) {
        let Some(item) = self.navigation.item().cloned() else {
            return;
        };
        match item {
            Item::Station(station) => wave::start(vec![station.seed]),
            Item::Track(track) => {
                let (tracks, placement) = match placement {
                    Placement::Replace(_) => {
                        let (tracks, start) = self.navigation.tracks_from_selection();
                        (tracks, Placement::Replace(start))
                    }
                    other => (vec![track], other),
                };
                self.background(move |session| queue::put(session, &tracks, placement), Browser::finished);
            }
            item => {
                let Some(view) = View::of(&item) else {
                    return;
                };
                self.background(
                    move |session| queue::put(session, &session.service()?.page(&view)?.tracks(), placement),
                    Browser::finished,
                );
            }
        }
    }

    fn finished(&mut self, result: Result<()>) {
        self.report(result);
    }

    fn open_item(&mut self) {
        let Some(item) = self.navigation.item().cloned() else {
            return;
        };
        match &item {
            Item::Track(track) => self.open(View::Track(track.id.clone()), track.full_title(), false),
            Item::Link(link) => controls::open_url(&link.url),
            Item::Concert(concert) => controls::open_url(&concert.url),
            item => {
                if let Some(view) = View::of(item) {
                    self.open(view, text::title(item), false);
                }
            }
        }
    }

    fn radio(&mut self) {
        if let Some(seed) = self.navigation.item().and_then(Item::seed) {
            wave::start(vec![seed]);
        }
    }

    fn relation(&mut self, relation: Relation) {
        let Some(target) = self.navigation.item().and_then(Item::target) else {
            return;
        };
        self.background(
            move |session| actions::toggle(session, &target, relation),
            |browser, result| {
                browser.report(result);
                browser.update_buttons();
            },
        );
    }

    fn more(&mut self) {
        let (Some(place), Some(section)) = (self.navigation.current(), self.navigation.section()) else {
            return;
        };
        let Some(page) = section.more else {
            return;
        };
        let (view, kind) = (place.view.clone(), section.kind);
        let requested = view.clone();
        self.background(
            move |session| session.service()?.more(&requested, kind, page),
            move |browser, result| {
                if let Some(paged) = browser.report(result) {
                    let added = browser.navigation.extend(&view, kind, paged);
                    browser.render();
                    speech::say(text_with("winamp-track-count", &[("count", &added.to_string())]));
                }
            },
        );
    }

    fn back(&mut self) {
        if self.navigation.back() {
            self.render();
            controls::focus(self.form.list.control);
        }
    }
}
