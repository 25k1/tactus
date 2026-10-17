mod actions;
mod bar;
mod browser;
mod command;
mod dialogs;
mod events;
mod login;
mod lyrics;
mod media_list;
mod navigation;
mod remote;
mod settings_dialog;
mod sounds;
mod text;
mod tray;
mod view_model;
mod voice;
mod widgets;

use crate::platform::hotkeys::{HotkeyEvent, Hotkeys};
use crate::speech::Speech;
use bar::PlayerBar;
use browser::Browser;
use command::Command;
use login::LoginPanel;
use lyrics::LyricsPanel;
use navigation::{History, Location, Section};
use sounds::Sounds;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tactus_api::Playlist;
use tactus_core::announce::Announcement;
use tactus_core::hotkey::{GlobalAction, Hotkey};
use tactus_core::player::{Connect, Player};
use tactus_core::remote::{RemoteCommand, RemoteHub, RemoteServer};
use tactus_core::settings::Settings;
use view_model::Content;
use widgets::*;
use wxdragon::prelude::*;
use wxdragon::timer::Timer;
use wxdragon::widgets::simplebook::SimpleBook;

const TICK_MS: i32 = 100;
const SUGGEST_DELAY: Duration = Duration::from_millis(350);
const BROWSER_PAGE: usize = 0;
const LYRICS_PAGE: usize = 1;

pub struct Ui {
    player: RefCell<Player>,
    frame: Frame,
    speech: Speech,
    sounds: Sounds,
    login: LoginPanel,
    main: Panel,
    sidebar: ListBox,
    book: SimpleBook,
    browser: Browser,
    lyrics: LyricsPanel,
    bar: PlayerBar,
    location: RefCell<Location>,
    history: RefCell<History>,
    content: RefCell<Content>,
    context_rows: RefCell<Vec<usize>>,
    restore: Cell<Option<(usize, usize)>>,
    focus_results: Cell<bool>,
    playlist_menu: RefCell<Vec<Playlist>>,
    tray: RefCell<Option<TaskBarIcon>>,
    quitting: Cell<bool>,
    remote_hub: Arc<RemoteHub>,
    remote_commands: Receiver<RemoteCommand>,
    remote_server: RefCell<Option<RemoteServer>>,
    hotkeys: RefCell<Option<Hotkeys>>,
    hotkey_sender: Sender<HotkeyEvent>,
    hotkey_events: Receiver<HotkeyEvent>,
    pending_suggestion: RefCell<Option<(String, Instant)>>,
    timer: RefCell<Option<Timer<Frame>>>,
    voice: RefCell<tactus_core::assistant::Assistant>,
}

pub fn run() {
    let _ = main(|_| {
        std::mem::forget(crate::platform::instance::acquire());
        tactus_core::log::init(tactus_core::settings::data_dir().join("tactus.log"));
        let settings = Settings::load();
        crate::i18n::init(settings.language.as_deref());
        tactus_api::net::set_language(&crate::i18n::current_language());
        tactus_core::log::info(format_args!("Tactus {} started", tactus_core::update::CURRENT));
        tactus_core::update::cleanup();
        #[cfg(feature = "fake")]
        let connect: Connect = |_| Ok(Arc::new(tactus_api::fake::FakeService::demo()));
        #[cfg(not(feature = "fake"))]
        let connect: Connect = tactus_core::player::connect_yandex;
        let ui = Ui::build(Player::new(settings, connect));
        crate::platform::instance::listen();
        ui.bind();
        ui.start();
    });
}

impl Ui {
    fn build(player: Player) -> Rc<Ui> {
        let frame = Frame::builder().with_title("Tactus").with_size(Size::new(1180, 780)).build();
        if let Some(icon) = app_icon(32) {
            frame.set_icon(&icon);
        }
        frame.create_status_bar(1, 0, ID_ANY as Id, "status");
        frame.set_menu_bar(command::menu_bar(&command::Keymap::new(&player.settings.app_keys)));

        let root = Panel::builder(&frame).build();
        let root_sizer = column();
        let login = LoginPanel::new(&root);

        let main = Panel::builder(&root).build();
        let sidebar_label = label(&main, &crate::i18n::fl!("sidebar-name"));
        let sidebar = ListBox::builder(&main)
            .with_choices(Section::ALL.iter().map(|section| section.label()).collect())
            .with_size(Size::new(210, -1))
            .build();
        let book = SimpleBook::builder(&main).build();
        let browser = Browser::new(&book, &player.settings.columns);
        let lyrics = LyricsPanel::new(&book);
        book.add_page(&browser.panel, "", true, None);
        book.add_page(&lyrics.panel, "", false, None);
        let bar = PlayerBar::new(&main, player.settings.volume);

        let sidebar_column = column();
        sidebar_column.add(&sidebar_label, 0, SizerFlag::All, 4);
        sidebar_column.add(&sidebar, 1, SizerFlag::Expand | SizerFlag::All, 4);
        let content_row = row();
        content_row.add_sizer(&sidebar_column, 0, SizerFlag::Expand, 0);
        content_row.add(&book, 1, SizerFlag::Expand | SizerFlag::All, 4);
        let main_sizer = column();
        main_sizer.add_sizer(&content_row, 1, SizerFlag::Expand, 0);
        main_sizer.add(&bar.panel, 0, SizerFlag::Expand | SizerFlag::All, 4);
        main.set_sizer(main_sizer, true);

        root_sizer.add(&login.panel, 1, SizerFlag::Expand, 0);
        root_sizer.add(&main, 1, SizerFlag::Expand, 0);
        root.set_sizer(root_sizer, true);

        let (remote_hub, remote_commands) = RemoteHub::new();
        let (hotkey_sender, hotkey_events) = mpsc::channel();
        Rc::new(Ui {
            player: RefCell::new(player),
            frame,
            speech: Speech::new(),
            sounds: Sounds::new(),
            login,
            main,
            sidebar,
            book,
            browser,
            lyrics,
            bar,
            location: RefCell::new(Location::Section(Section::Search)),
            history: RefCell::new(History::default()),
            content: RefCell::new(Content::default()),
            context_rows: RefCell::new(Vec::new()),
            restore: Cell::new(None),
            focus_results: Cell::new(false),
            playlist_menu: RefCell::new(Vec::new()),
            tray: RefCell::new(None),
            quitting: Cell::new(false),
            remote_hub,
            remote_commands,
            remote_server: RefCell::new(None),
            hotkeys: RefCell::new(None),
            hotkey_sender,
            hotkey_events,
            pending_suggestion: RefCell::new(None),
            timer: RefCell::new(None),
            voice: RefCell::default(),
        })
    }

    fn settings(&self) -> Settings {
        self.player.borrow().settings.clone()
    }

    fn act(self: &Rc<Self>, action: impl FnOnce(&mut Player)) {
        if let Ok(mut player) = self.player.try_borrow_mut() {
            action(&mut player);
        } else {
            return;
        }
        self.apply_events();
    }

    fn set_status(&self, text: &str) {
        self.frame.set_status_text(text, 0);
    }

    fn announce(&self, kind: Announcement, text: &str, interrupt: bool) {
        let enabled = self.player.borrow().settings.announcements.contains(&kind);
        if enabled && !text.is_empty() {
            self.speech.say(text, interrupt);
        }
    }

    fn cue(&self, cue: tactus_core::player::Cue) {
        if self.player.borrow().settings.sound_cues {
            self.sounds.play(cue);
        }
    }

    fn start(self: &Rc<Self>) {
        let settings = self.settings();
        self.player.borrow_mut().attach_window(self.frame.get_handle() as usize);
        self.set_tray(settings.tray);
        self.set_remote(settings.remote_enabled);
        self.set_hotkeys();
        self.start_voice();
        if settings.notifications || settings.episode_notifications {
            crate::platform::notify::enable();
        }
        let timer = Timer::new(&self.frame);
        let ui = self.clone();
        timer.on_tick(move |_| ui.tick());
        timer.start(TICK_MS, false);
        *self.timer.borrow_mut() = Some(timer);
        self.navigate(Location::Section(Section::Search), false);
        self.act(Player::start);
        if crate::platform::instance::after_update() {
            let text = crate::i18n::fl!("update-installed", version = tactus_core::update::CURRENT);
            self.set_status(&text);
            self.announce(Announcement::Errors, &text, false);
        } else if settings.check_updates {
            self.act(|player| player.check_updates(false));
        }
        self.show_screen();
        self.frame.centre();
        let hidden = settings.start_minimized;
        if hidden && self.tray.borrow().is_some() {
            return;
        }
        self.frame.show(true);
        if hidden {
            self.frame.iconize(true);
        }
        self.focus_content();
    }

    fn tick(self: &Rc<Self>) {
        if let Ok(mut player) = self.player.try_borrow_mut() {
            player.poll();
        }
        self.apply_events();
        let commands: Vec<RemoteCommand> = self.remote_commands.try_iter().collect();
        for command in commands {
            self.handle_remote(command);
        }
        let hotkeys: Vec<HotkeyEvent> = self.hotkey_events.try_iter().collect();
        for event in hotkeys {
            match event {
                HotkeyEvent::Pressed(GlobalAction::Voice) => self.voice_pressed(),
                HotkeyEvent::Released(GlobalAction::Voice) => self.voice_released(),
                HotkeyEvent::Pressed(action) => self.execute(Command::from_global(action)),
                HotkeyEvent::Released(_) => {}
            }
        }
        self.poll_voice();
        if crate::platform::instance::take_raise_request() {
            self.raise_window();
        }
        let due = self.pending_suggestion.borrow().as_ref().is_some_and(|(_, at)| at.elapsed() >= SUGGEST_DELAY);
        if let Some((text, _)) = due.then(|| self.pending_suggestion.borrow_mut().take()).flatten() {
            self.act(|player| player.suggest(&text));
        }
        let minimize = self.player.borrow().settings.minimize_to_tray;
        if minimize && self.frame.is_iconized() && self.frame.is_shown() && self.tray.borrow().is_some() {
            self.frame.show(false);
        }
    }

    fn shutdown(&self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.stop();
        }
        self.player.borrow_mut().shutdown();
        self.remote_server.borrow_mut().take();
        self.hotkeys.borrow_mut().take();
        self.set_tray_visible(false);
    }

    fn show_screen(&self) {
        let signed_in = self.player.borrow().is_signed_in();
        self.login.panel.show(!signed_in);
        self.main.show(signed_in);
        self.update_login();
        if let Some(parent) = self.main.get_parent() {
            parent.layout();
        }
    }

    fn update_login(&self) {
        self.login.update(&self.player.borrow().login);
    }

    fn set_remote(&self, enabled: bool) {
        self.remote_server.borrow_mut().take();
        if !enabled {
            return;
        }
        let settings = self.settings();
        match RemoteServer::start(settings.remote_port, settings.remote_token.clone(), self.remote_hub.clone()) {
            Ok(server) => {
                *self.remote_server.borrow_mut() = Some(server);
                self.publish_status();
                self.publish_queue();
                self.set_status(&crate::i18n::fl!("remote-listening", port = settings.remote_port.to_string()));
            }
            Err(error) => self.set_status(&crate::i18n::fl!("remote-error", error = error.to_string())),
        }
    }

    fn set_hotkeys(&self) {
        self.hotkeys.borrow_mut().take();
        let settings = self.settings();
        if !settings.global_hotkeys {
            return;
        }
        let bindings: Vec<(GlobalAction, Hotkey)> = GlobalAction::ALL
            .into_iter()
            .filter_map(|action| Some((action, Hotkey::parse(&action.key_in(&settings.hotkeys)?)?)))
            .collect();
        let (hotkeys, failed) = Hotkeys::start(bindings, self.hotkey_sender.clone());
        *self.hotkeys.borrow_mut() = Some(hotkeys);
        if !failed.is_empty() {
            let keys: Vec<String> = failed.iter().filter_map(|action| action.key_in(&settings.hotkeys)).collect();
            self.set_status(&crate::i18n::fl!("hotkeys-failed", keys = keys.join(", ")));
        }
    }
}
