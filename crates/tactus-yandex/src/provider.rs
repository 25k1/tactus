use crate::auth;
use crate::service::YandexService;
use std::sync::Arc;
use std::time::Duration;
use tactus_api::error::{Error, Result};
use tactus_api::{Capabilities, Destination, Item, Login, LoginPrompt, Provider, ProviderKind, Service};

const CAPABILITIES: Capabilities = Capabilities::WAVE
    .union(Capabilities::GENERATED)
    .union(Capabilities::DISLIKE)
    .union(Capabilities::PINS)
    .union(Capabilities::UPLOAD)
    .union(Capabilities::DOWNLOAD)
    .union(Capabilities::LYRICS)
    .union(Capabilities::PRESAVE)
    .union(Capabilities::CONCERTS)
    .union(Capabilities::COLLECTIVE)
    .union(Capabilities::TRAILER)
    .union(Capabilities::ACCOUNT_SETTINGS)
    .union(Capabilities::PROMO_CODES)
    .union(Capabilities::REMOTE_QUEUE)
    .union(Capabilities::YNISON)
    .union(Capabilities::CHARTS)
    .union(Capabilities::PODCASTS);

pub struct YandexProvider;

struct DeviceLogin {
    prompt: LoginPrompt,
    device_code: String,
}

impl Login for DeviceLogin {
    fn prompt(&self) -> &LoginPrompt {
        &self.prompt
    }

    fn poll(&mut self) -> Result<Option<String>> {
        match auth::poll_device_token(&self.device_code) {
            Ok(token) => Ok(Some(token)),
            Err(Error::AuthorizationPending) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Provider for YandexProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Yandex
    }

    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn login(&self) -> Result<Box<dyn Login>> {
        let device = auth::request_device_code()?;
        let prompt = LoginPrompt {
            code: device.user_code,
            url: device.verification_url,
            interval: Duration::from_secs(device.interval.max(2)),
            expires: Duration::from_secs(device.expires_in.max(60)),
        };
        Ok(Box::new(DeviceLogin { prompt, device_code: device.device_code }))
    }

    fn connect(&self, credentials: &str) -> Result<Service> {
        Ok(Arc::new(YandexService::new(credentials)?))
    }

    fn destination(&self, link: &str) -> Option<Destination> {
        crate::link::destination(link)
    }

    fn share_link(&self, item: &Item) -> Option<String> {
        crate::link::share(item)
    }
}
