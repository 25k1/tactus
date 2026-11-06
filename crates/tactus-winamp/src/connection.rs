use crate::store::Store;
use std::sync::Mutex;
use tactus_api::error::{Error, Result};
use tactus_api::sync::Guard;
use tactus_api::{Provider, Service, Track};
use tactus_yandex::YandexProvider;

pub struct Connection {
    store: Store,
    cached: Mutex<Option<(String, Service)>>,
}

impl Connection {
    pub fn new(store: Store) -> Self {
        Self { store, cached: Mutex::new(None) }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn service(&self) -> Result<Service> {
        let token = self.store.token().ok_or(Error::SignedOut)?;
        let mut cached = self.cached.guard();
        if let Some((known, service)) = cached.as_ref()
            && *known == token
        {
            return Ok(service.clone());
        }
        let service = YandexProvider.connect(&token, None)?;
        *cached = Some((token, service.clone()));
        Ok(service)
    }

    pub fn track(&self, id: &str) -> Result<Track> {
        let ids = [id.to_owned()];
        self.service()?.tracks(&ids)?.into_iter().next().ok_or_else(|| Error::NotFound(id.to_owned()))
    }

    pub fn signed_in(&self) -> bool {
        self.store.token().is_some()
    }

    pub fn sign_in(&self, token: &str) -> Result<Service> {
        self.store.save_token(token)?;
        self.service()
    }

    pub fn sign_out(&self) -> Result<()> {
        *self.cached.guard() = None;
        self.store.forget()
    }
}
