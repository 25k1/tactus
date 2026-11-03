use crate::capabilities::Capabilities;
use crate::error::Result;
use crate::model::{Destination, Item};
use crate::service::Service;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[default]
    Yandex,
    Spotify,
}

impl ProviderKind {
    pub const fn name(self) -> &'static str {
        match self {
            ProviderKind::Yandex => "Yandex Music",
            ProviderKind::Spotify => "Spotify",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoginPrompt {
    pub code: String,
    pub url: String,
    pub interval: Duration,
    pub expires: Duration,
}

pub trait Login: Send {
    fn prompt(&self) -> &LoginPrompt;
    fn poll(&mut self) -> Result<Option<String>>;
}

pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn capabilities(&self) -> Capabilities;
    fn login(&self) -> Result<Box<dyn Login>>;
    fn connect(&self, credentials: &str) -> Result<Service>;
    fn destination(&self, _link: &str) -> Option<Destination> {
        None
    }

    fn share_link(&self, item: &Item) -> Option<String> {
        match item {
            Item::Link(link) => Some(link.url.clone()),
            Item::Concert(concert) => Some(concert.url.clone()),
            _ => None,
        }
    }
}
