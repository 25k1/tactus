use std::sync::{Mutex, MutexGuard, OnceLock};
use tactus_api::error::Result;
use tactus_api::sync::Guard;
use tactus_api::{Account, Library, Service, net};
use tactus_winamp::{Connection, Store, Winamp, log, speech, text};
use windows_sys::Win32::Foundation::HINSTANCE;

#[derive(Default)]
pub struct State {
    pub library: Library,
    pub account: Option<Account>,
    pub wave: Option<crate::wave::Wave>,
}

pub struct Session {
    pub winamp: Winamp,
    pub instance: isize,
    pub connection: Connection,
    state: Mutex<State>,
}

static SESSION: OnceLock<Session> = OnceLock::new();

pub fn init(winamp: Winamp, instance: HINSTANCE, store: Store) {
    let _ = SESSION.set(Session {
        winamp,
        instance: instance as isize,
        connection: Connection::new(store),
        state: Mutex::default(),
    });
}

pub fn get() -> Option<&'static Session> {
    SESSION.get()
}

impl Session {
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.guard()
    }

    pub fn store(&self) -> &Store {
        self.connection.store()
    }

    pub fn service(&self) -> Result<Service> {
        self.connection.service()
    }

    pub fn instance(&self) -> HINSTANCE {
        self.instance as HINSTANCE
    }

    pub fn account_name(&self) -> Option<String> {
        let state = self.state();
        let account = state.account.as_ref()?;
        Some(if account.name.is_empty() { account.login.clone() } else { account.name.clone() })
    }

    pub fn refresh(&'static self, done: impl FnOnce() + Send + 'static) {
        if !self.connection.signed_in() {
            return;
        }
        run(move || {
            let service = self.service()?;
            let account = service.account()?;
            self.state().account = Some(account);
            done();
            let library = service.library()?;
            self.state().library = library;
            Ok(())
        });
    }

    pub fn sign_out(&self) -> Result<()> {
        self.connection.sign_out()?;
        self.signed_out();
        Ok(())
    }

    fn signed_out(&self) {
        let mut state = self.state();
        state.account = None;
        state.library = Library::default();
        state.wave = None;
    }
}

pub fn run(task: impl FnOnce() -> Result<()> + Send + 'static) {
    net::spawn_blocking(move || {
        if let Err(error) = task() {
            log::write(&error);
            speech::say(text::error(&error));
        }
    });
}
