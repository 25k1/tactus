use super::*;

impl Ui {
    pub(crate) fn bind(self: &Rc<Self>) {
        let ui = self.clone();
        self.frame.on_menu(move |event| ui.on_menu(event.get_id()));
        let ui = self.clone();
        self.frame.on_close(move |event| {
            if let WindowEventData::General(general) = &event
                && !ui.quitting.get()
                && ui.tray.borrow().is_some()
                && general.can_veto()
            {
                general.veto();
                ui.frame.show(false);
                return;
            }
            ui.shutdown();
            event.skip(true);
        });
        self.bind_login();
        self.bind_browser();
        self.bind_bar();
        let ui = self.clone();
        self.sidebar.on_selection_changed(move |event| {
            let section = event.get_selection().and_then(|index| Section::ALL.get(index.max(0) as usize).copied());
            if let Some(section) = section {
                ui.navigate(Location::Section(section), false);
            }
        });
        let ui = self.clone();
        WindowEvents::on_key_down(&self.sidebar, move |event| match key(&event) {
            Some(WXK_RETURN) => ui.focus_content(),
            _ => event.skip(true),
        });
        let ui = self.clone();
        WindowEvents::on_key_down(&self.lyrics.lines, move |event| match key(&event) {
            Some(WXK_RETURN) => ui.seek_to_lyric(),
            _ => event.skip(true),
        });
    }

    pub(super) fn bind_login(self: &Rc<Self>) {
        let ui = self.clone();
        self.login.get_code.on_click(move |_| ui.act(Player::begin_login));
        let ui = self.clone();
        self.login.copy_code.on_click(move |_| {
            let code = ui.login.code.get_value();
            if !code.is_empty() && Clipboard::get().set_text(&code) {
                ui.speech.say(&fl!("login-code-copied"), true);
            }
        });
        let ui = self.clone();
        self.login.open_page.on_click(move |_| {
            if let tactus_core::player::LoginState::Waiting { url, .. } = &ui.player.borrow().login {
                launch_default_browser(url, BrowserLaunchFlags::Default);
            }
        });
    }

    pub(super) fn bind_browser(self: &Rc<Self>) {
        let browser = &self.browser;
        let ui = self.clone();
        browser.filter.on_text_updated(move |_| ui.render(None, Some(0)));
        let ui = self.clone();
        browser.sort.on_selection_changed(move |_| ui.render(None, Some(0)));
        let ui = self.clone();
        WindowEvents::on_key_down(&browser.filter, move |event| match key(&event) {
            Some(WXK_ESCAPE) => {
                ui.browser.filter.change_value("");
                ui.render(None, Some(0));
                ui.browser.list.list.set_focus();
            }
            Some(WXK_RETURN) => ui.browser.list.list.set_focus(),
            _ => event.skip(true),
        });
        let ui = self.clone();
        browser.search.on_enter_pressed(move |_| ui.run_search());
        let ui = self.clone();
        browser.search_button.on_click(move |_| ui.run_search());
        let ui = self.clone();
        browser.search.on_selection_changed(move |_| ui.run_search());
        let ui = self.clone();
        browser.search.on_text_updated(move |_| {
            let text = ui.browser.search.get_value().trim().to_owned();
            if text.is_empty() {
                ui.show_search_history();
            }
            *ui.pending_suggestion.borrow_mut() = (text.chars().count() >= 2).then(|| (text, Instant::now()));
        });
        let ui = self.clone();
        browser.wave_start.on_click(move |_| {
            let seeds = ui.browser.wave_seeds();
            ui.act(|player| player.start_wave(seeds));
        });
        for (kind, control) in &browser.buttons {
            let (ui, kind) = (self.clone(), *kind);
            control.on_click(move |_| ui.on_page_button(kind));
        }
        let ui = self.clone();
        browser.section.on_selection_changed(move |_| {
            let section = ui.browser.section_index();
            ui.render(Some(section), Some(0));
            ui.remember();
        });
        let list = browser.list.list;
        let ui = self.clone();
        list.on_item_activated(move |event| {
            if let Ok(row) = usize::try_from(event.get_item_index()) {
                ui.activate(row);
            }
        });
        let ui = self.clone();
        list.on_item_selected(move |event| {
            if let Ok(row) = usize::try_from(event.get_item_index()) {
                ui.load_more_near(row);
            }
        });
        let ui = self.clone();
        list.bind_internal(EventType::CONTEXT_MENU, move |_| {
            if let Some(row) = ui.browser.list.selected() {
                ui.show_context_menu(row);
            }
        });
        let ui = self.clone();
        list.on_key_down(move |event| ui.list_key(event.get_key_code()));
    }

    pub(super) fn bind_bar(self: &Rc<Self>) {
        let bar = &self.bar;
        self.bind_player_menu(&bar.panel);
        for (control, command) in [
            (&bar.toggle, Command::PlayerControls),
            (&bar.previous, Command::Previous),
            (&bar.play, Command::PlayPause),
            (&bar.next, Command::Next),
        ] {
            let ui = self.clone();
            control.on_click(move |_| ui.execute(command));
        }
        let ui = self.clone();
        bar.position.on_slider(move |event| {
            let seconds = event.get_value().max(0) as u64;
            ui.act(|player| player.seek(seconds * 1000));
        });
        let ui = self.clone();
        bar.volume.on_slider(move |event| {
            let volume = event.get_value() as i16;
            ui.act(|player| player.set_volume(volume));
        });
        let ui = self.clone();
        bar.repeat.on_selection_changed(move |_| {
            let repeat = bar::repeat_at(selection(&ui.bar.repeat));
            ui.act(|player| player.set_repeat(repeat));
        });
        let ui = self.clone();
        bar.shuffle.on_toggled(move |event| {
            let shuffle = event.is_checked();
            ui.act(|player| player.set_shuffle(shuffle));
        });
    }

    pub(super) fn list_key(self: &Rc<Self>, code: Option<i32>) {
        match (code, self.browser.list.selected()) {
            (Some(WXK_BACK), _) => self.execute(Command::Back),
            (Some(WXK_SPACE), _) if !get_key_state(WXK_CONTROL) => self.execute(Command::PlayPause),
            (Some(WXK_DELETE), Some(_)) => self.remove_selected(),
            _ => {}
        }
    }

    pub(super) fn load_more_near(self: &Rc<Self>, row: usize) {
        const AHEAD: usize = 5;
        let section = self.browser.section_index();
        if row + AHEAD < self.browser.list.len() {
            return;
        }
        if let Some(view) = self.current_view()
            && self.player.borrow().has_more(&view, section)
        {
            self.act(|player| player.load_more(&view, section));
        }
    }
}
