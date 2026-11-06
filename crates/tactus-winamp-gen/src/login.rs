use crate::session::Session;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use tactus_api::error::{Error, Result};
use tactus_api::{LoginPrompt, Provider, net};
use tactus_yandex::YandexProvider;

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub enum Event {
    Prompt(LoginPrompt),
    Finished(Result<()>),
}

pub fn cancel() {
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

pub fn start(session: &'static Session, notify: impl Fn(Event) + Send + 'static) {
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let current = move || GENERATION.load(Ordering::Acquire) == generation;
    net::spawn_blocking(move || {
        if let Some(result) = poll(session, &current, &notify) {
            notify(Event::Finished(result));
        }
    });
}

fn poll(session: &Session, current: &dyn Fn() -> bool, notify: &dyn Fn(Event)) -> Option<Result<()>> {
    let mut login = match YandexProvider.login() {
        Ok(login) => login,
        Err(error) => return Some(Err(error)),
    };
    let prompt = login.prompt().clone();
    notify(Event::Prompt(prompt.clone()));
    let deadline = Instant::now() + prompt.expires;
    while Instant::now() < deadline {
        std::thread::sleep(prompt.interval);
        if !current() {
            return None;
        }
        match login.poll() {
            Ok(Some(token)) => return Some(session.connection.sign_in(&token).map(drop)),
            Ok(None) => {}
            Err(error) => return Some(Err(error)),
        }
    }
    current().then_some(Err(Error::Expired))
}
