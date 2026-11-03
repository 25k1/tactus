use crate::settings;
use serde::{Deserialize, Serialize};
use tactus_api::error::Result;
use tactus_api::proxy::Proxy;
use tactus_api::{ProviderKind, Quality};

const FILE: &str = "accounts.json";
const TOKEN: &str = "token";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountEntry {
    pub id: String,
    pub provider: ProviderKind,
    pub alias: String,
    pub stream_quality: Option<Quality>,
    pub download_quality: Option<Quality>,
    pub proxy: Option<Proxy>,
}

impl AccountEntry {
    pub fn new(provider: ProviderKind, existing: &[AccountEntry]) -> Self {
        let prefix = match provider {
            ProviderKind::Yandex => "yandex",
            ProviderKind::Spotify => "spotify",
        };
        let id = (1..)
            .map(|number| format!("{prefix}-{number}"))
            .find(|id| existing.iter().all(|entry| entry.id != *id))
            .unwrap_or_default();
        Self { id, provider, ..Self::default() }
    }

    pub fn file(&self, name: &str) -> String {
        format!("accounts/{}/{name}", self.id)
    }

    pub fn load_token(&self) -> Option<String> {
        settings::read_text(&self.file(TOKEN)).map(|token| token.trim().to_owned()).filter(|token| !token.is_empty())
    }

    pub fn save_token(&self, token: &str) -> Result<()> {
        settings::write_text(&self.file(TOKEN), token)
    }

    pub fn forget(&self) -> Result<()> {
        settings::remove_dir(&format!("accounts/{}", self.id))
    }

    pub fn is_saved(&self) -> bool {
        !self.id.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountList {
    pub accounts: Vec<AccountEntry>,
    pub active: String,
    pub queue_owner: String,
}

impl AccountList {
    pub fn load() -> Self {
        settings::read_json(FILE).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        settings::write_json(FILE, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_new_accounts_per_provider() {
        let first = AccountEntry::new(ProviderKind::Yandex, &[]);
        assert_eq!(first.id, "yandex-1");
        let second = AccountEntry::new(ProviderKind::Yandex, std::slice::from_ref(&first));
        assert_eq!(second.id, "yandex-2");
        assert_eq!(AccountEntry::new(ProviderKind::Spotify, &[first, second]).id, "spotify-1");
    }

    #[test]
    fn keeps_account_files_in_their_own_folder() {
        let entry = AccountEntry { id: "yandex-3".into(), ..AccountEntry::default() };
        assert_eq!(entry.file("session.json"), "accounts/yandex-3/session.json");
        assert!(!AccountEntry::default().is_saved());
    }

    #[test]
    fn account_list_round_trips() {
        let list = AccountList {
            accounts: vec![AccountEntry {
                id: "spotify-1".into(),
                provider: ProviderKind::Spotify,
                alias: "Work".into(),
                stream_quality: Some(Quality::Low),
                ..AccountEntry::default()
            }],
            active: "spotify-1".into(),
            queue_owner: "spotify-1".into(),
        };
        let text = serde_json::to_string(&list).unwrap();
        assert_eq!(serde_json::from_str::<AccountList>(&text).unwrap(), list);
    }
}
