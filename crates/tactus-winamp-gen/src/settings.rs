use crate::Action;
use crate::browser;
use crate::login::{self, Event};
use crate::session::{self, Session};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use tactus_api::sync::Guard;
use tactus_api::{LoginPrompt, Quality};
use tactus_winamp::text::{self, text, text_with};
use tactus_winamp::ui::controls;
use tactus_winamp::ui::dialog::{self, CANCEL, Layout, OK, Page};
use tactus_winamp::{Language, speech, winamp};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

const WIDTH: i32 = 420;
const ACCOUNT_HEIGHT: i32 = 48;
const HOTKEYS_HEIGHT: i32 = 110;
const QUALITY: u16 = 100;
const LANGUAGE: u16 = 101;
const ACCOUNT: u16 = 102;
const ACCOUNT_BUTTON: u16 = 103;
const COPY_CODE: u16 = 104;
const OPEN_PAGE: u16 = 105;
const HOTKEYS: u16 = 106;

const RESERVED: isize = -1;

static OPEN: AtomicIsize = AtomicIsize::new(0);

enum Update {
    Login(Event),
    Account,
}

type Inbox = Arc<Mutex<Vec<Update>>>;

struct Settings {
    session: &'static Session,
    dialog: isize,
    inbox: Inbox,
    prompt: Option<LoginPrompt>,
    quality: HWND,
    language: HWND,
    account: HWND,
    account_button: HWND,
    copy_code: HWND,
    open_page: HWND,
}

pub fn quality_label(quality: Quality) -> String {
    text(match quality {
        Quality::Lossless => "quality-lossless",
        Quality::High => "quality-high",
        Quality::Low => "quality-low",
    })
}

fn language_label(language: Language) -> String {
    language.code().map_or_else(|| text("winamp-language-automatic"), text::language_name)
}

fn position<T: PartialEq>(all: &[T], value: &T) -> usize {
    all.iter().position(|known| known == value).unwrap_or(0)
}

fn hotkeys() -> String {
    let actions = Action::ALL.iter().map(|action| text::plain(&action.label()));
    std::iter::once(text("winamp-hotkeys-help")).chain(actions).collect::<Vec<_>>().join("\r\n")
}

fn post(inbox: &Inbox, dialog: isize, update: Update) {
    inbox.guard().push(update);
    dialog::wake(dialog);
}

impl Settings {
    fn say(&self, message: &str) {
        controls::set_text(self.account, message);
        speech::say(message);
    }

    fn show_account(&self) {
        let signed_in = self.session.connection.signed_in();
        let status = if signed_in {
            text_with("status-signed-in", &[("name", &self.session.account_name().unwrap_or_default())])
        } else {
            text("winamp-signed-out")
        };
        controls::set_text(self.account, &status);
        controls::set_text(self.account_button, &text(if signed_in { "winamp-sign-out" } else { "winamp-sign-in" }));
    }

    fn show_login(&self, visible: bool) {
        controls::show(self.copy_code, visible);
        controls::show(self.open_page, visible);
    }

    fn sign_in(&mut self) {
        self.say(&text("login-requesting"));
        let (inbox, dialog) = (self.inbox.clone(), self.dialog);
        login::start(self.session, move |event| post(&inbox, dialog, Update::Login(event)));
    }

    fn sign_out(&mut self, dialog: HWND) {
        let name = self.session.account_name().unwrap_or_default();
        if !winamp::ask(dialog, &text_with("winamp-sign-out-confirm", &[("account", &name)])) {
            return;
        }
        if let Err(error) = self.session.sign_out() {
            return self.say(&text::error(&error));
        }
        self.show_account();
        browser::account_changed();
        controls::focus(self.account_button);
    }

    fn login_event(&mut self, event: Event) {
        match event {
            Event::Prompt(prompt) => {
                controls::copy(self.account, &prompt.code);
                controls::open_url(&prompt.url);
                self.say(&text_with("winamp-login-code", &[("code", &prompt.code)]));
                self.show_login(true);
                self.prompt = Some(prompt);
            }
            Event::Finished(result) => {
                self.prompt = None;
                self.show_login(false);
                match result {
                    Ok(()) => {
                        self.show_account();
                        browser::account_changed();
                        let (inbox, dialog) = (self.inbox.clone(), self.dialog);
                        self.session.refresh(move || post(&inbox, dialog, Update::Account));
                    }
                    Err(error) => self.say(&text::error(&error)),
                }
            }
        }
    }
}

impl Page for Settings {
    fn build(&mut self, layout: &mut Layout) -> HWND {
        self.dialog = layout.dialog() as isize;
        OPEN.store(self.dialog, Ordering::Release);
        let current = self.session.store().settings();
        self.quality = layout.line(&text("winamp-quality"), |parent| controls::combo(parent, QUALITY));
        let qualities = Quality::ALL.map(quality_label);
        controls::fill_combo(self.quality, &qualities, position(&Quality::ALL, &current.quality));
        self.language = layout.line(&text("winamp-language"), |parent| controls::combo(parent, LANGUAGE));
        let languages = Language::ALL.map(language_label);
        controls::fill_combo(self.language, &languages, position(&Language::ALL, &current.language));
        let account = |parent| controls::read_only(parent, ACCOUNT);
        self.account = layout.field(&text("winamp-account"), ACCOUNT_HEIGHT, account);
        let buttons = [
            (ACCOUNT_BUTTON, String::new()),
            (COPY_CODE, text("winamp-copy-code")),
            (OPEN_PAGE, text("winamp-open-page")),
        ];
        let buttons: Vec<(u16, &str)> = buttons.iter().map(|(id, label)| (*id, label.as_str())).collect();
        let created = layout.buttons(&buttons);
        (self.account_button, self.copy_code, self.open_page) = (created[0], created[1], created[2]);
        self.show_account();
        self.show_login(false);
        let hotkeys_text =
            layout.field(&text("winamp-hotkeys"), HOTKEYS_HEIGHT, |parent| controls::read_only(parent, HOTKEYS));
        controls::set_text(hotkeys_text, &hotkeys());
        layout.buttons(&[(OK, &text("dialog-ok")), (CANCEL, &text("dialog-cancel"))]);
        self.quality
    }

    fn command(&mut self, dialog: HWND, id: u16) {
        match id {
            ACCOUNT_BUTTON if self.session.connection.signed_in() => self.sign_out(dialog),
            ACCOUNT_BUTTON => self.sign_in(),
            COPY_CODE => {
                if let Some(prompt) = &self.prompt {
                    controls::copy(dialog, &prompt.code);
                    speech::say(text("login-code-copied"));
                }
            }
            OPEN_PAGE => {
                if let Some(prompt) = &self.prompt {
                    controls::open_url(&prompt.url);
                }
            }
            _ => {}
        }
    }

    fn accept(&mut self, dialog: HWND) -> bool {
        let pick = |combo, count: usize| controls::combo_selection(combo).filter(|index| *index < count);
        let quality = pick(self.quality, Quality::ALL.len()).map(|index| Quality::ALL[index]);
        let language = pick(self.language, Language::ALL.len()).map(|index| Language::ALL[index]);
        let saved = self.session.store().update_settings(|settings| {
            settings.quality = quality.unwrap_or(settings.quality);
            settings.language = language.unwrap_or(settings.language);
        });
        if let Err(error) = saved {
            winamp::message(dialog, &text::error(&error));
            return false;
        }
        true
    }

    fn update(&mut self, _dialog: HWND) {
        let updates = std::mem::take(&mut *self.inbox.guard());
        for update in updates {
            match update {
                Update::Login(event) => self.login_event(event),
                Update::Account => {
                    self.show_account();
                    speech::say(controls::text(self.account));
                }
            }
        }
    }
}

pub fn show(owner: HWND) {
    let Some(session) = session::get() else {
        return;
    };
    if let Err(open) = OPEN.compare_exchange(0, RESERVED, Ordering::AcqRel, Ordering::Acquire) {
        if open != RESERVED {
            unsafe { SetForegroundWindow(open as HWND) };
        }
        return;
    }
    let mut page = Settings {
        session,
        dialog: 0,
        inbox: Inbox::default(),
        prompt: None,
        quality: null_mut(),
        language: null_mut(),
        account: null_mut(),
        account_button: null_mut(),
        copy_code: null_mut(),
        open_page: null_mut(),
    };
    dialog::show(owner, &text("winamp-settings"), WIDTH, &mut page);
    OPEN.store(0, Ordering::Release);
    login::cancel();
}
