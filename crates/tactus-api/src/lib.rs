#![forbid(unsafe_code)]

pub mod auth;
pub mod error;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
pub mod model;
pub mod net;
pub mod parse;
mod service;
mod yandex;
pub mod ynison;

pub use model::*;
pub use service::*;
pub use yandex::YandexService;
