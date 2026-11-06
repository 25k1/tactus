#![allow(clippy::missing_safety_doc)]

pub mod bridge;
pub mod connection;
pub mod log;
pub mod sdk;
pub mod speech;
pub mod store;
pub mod text;
pub mod tracks;
pub mod ui;
pub mod wide;
pub mod winamp;

pub use connection::Connection;
pub use store::{Language, Settings, Store};
pub use winamp::Winamp;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = concat!("Yandex Music ", env!("CARGO_PKG_VERSION"), " (Tactus)\0");

pub fn prepare(winamp: Winamp) -> Store {
    let store = Store::locate(winamp);
    log::init(store.dir());
    let language = store.settings().language.code().unwrap_or_else(text::system_language);
    text::set_language(language);
    tactus_api::net::set_language(language);
    if let Some(proxy) = winamp.proxy().as_deref().and_then(winamp::parse_proxy) {
        tactus_api::net::set_proxy(&proxy);
    }
    store
}
