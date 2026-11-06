use crate::winamp::Winamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tactus_api::Quality;
use tactus_api::error::Result;

const FOLDER: &str = "tactus-yandex";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Automatic,
    English,
    Russian,
    Georgian,
}

impl Language {
    pub const ALL: [Language; 4] = [Language::Automatic, Language::English, Language::Russian, Language::Georgian];

    pub fn code(self) -> Option<&'static str> {
        match self {
            Language::Automatic => None,
            Language::English => Some("en"),
            Language::Russian => Some("ru"),
            Language::Georgian => Some("ka"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub quality: Quality,
    pub language: Language,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn locate(winamp: Winamp) -> Self {
        let base = winamp
            .ini_directory()
            .or_else(|| std::env::var_os("APPDATA").map(|appdata| Path::new(&appdata).join("Winamp")))
            .unwrap_or_else(std::env::temp_dir);
        Self::new(base.join("Plugins").join(FOLDER))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn write(&self, name: &str, contents: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.path(name), contents)?;
        Ok(())
    }

    pub fn token(&self) -> Option<String> {
        let token = std::fs::read_to_string(self.path("token")).ok()?;
        let token = token.trim();
        (!token.is_empty()).then(|| token.to_owned())
    }

    pub fn save_token(&self, token: &str) -> Result<()> {
        self.write("token", token.as_bytes())
    }

    pub fn forget(&self) -> Result<()> {
        match std::fs::remove_file(self.path("token")) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        }
    }

    pub fn settings(&self) -> Settings {
        std::fs::read(self.path("settings.json"))
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.write("settings.json", &serde_json::to_vec_pretty(settings)?)
    }

    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) -> Result<()> {
        let mut settings = self.settings();
        change(&mut settings);
        self.save_settings(&settings)
    }

    pub fn save_playlist(&self, contents: &str) -> Result<PathBuf> {
        self.write("queue.m3u8", contents.as_bytes())?;
        Ok(self.path("queue.m3u8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_token_and_settings_in_its_folder() {
        let dir = std::env::temp_dir().join(format!("tactus-winamp-store-{}", std::process::id()));
        let store = Store::new(dir.clone());
        assert_eq!(store.token(), None);
        store.save_token(" secret \n").unwrap();
        assert_eq!(store.token().as_deref(), Some("secret"));
        store.save_settings(&Settings { quality: Quality::Low, language: Language::Georgian }).unwrap();
        store.update_settings(|settings| settings.quality = Quality::High).unwrap();
        assert_eq!(store.settings(), Settings { quality: Quality::High, language: Language::Georgian });
        store.forget().unwrap();
        store.forget().unwrap();
        assert_eq!(store.token(), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
