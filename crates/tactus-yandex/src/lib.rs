#![forbid(unsafe_code)]

mod auth;
mod link;
mod parse;
mod provider;
mod service;
pub mod ynison;

pub use link::destination;
pub use provider::YandexProvider;
pub use service::YandexService;
