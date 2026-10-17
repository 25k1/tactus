use crate::settings::{self, app_dir, portable_path};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tactus_api::error::Result;
use tactus_api::{AudioSource, Codec, Track};

const FILE: &str = "downloads.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub track: Track,
    pub path: String,
}

impl Stored {
    pub fn file(&self) -> PathBuf {
        app_dir().join(&self.path)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Offline(BTreeMap<String, Stored>);

impl Offline {
    pub fn load() -> Self {
        let mut offline: Self = settings::read_json(FILE).unwrap_or_default();
        offline.0.retain(|_, stored| stored.file().is_file());
        offline
    }

    pub fn save(&self) -> Result<()> {
        settings::write_json(FILE, self)
    }

    pub fn add(&mut self, track: &Track, path: &Path) {
        let path = portable_path(&path.to_string_lossy());
        self.0.insert(track.id.clone(), Stored { track: Track { played: false, ..track.clone() }, path });
    }

    pub fn has(&self, id: &str) -> bool {
        self.0.contains_key(id)
    }

    pub fn source(&self, id: &str) -> Option<AudioSource> {
        let file = self.0.get(id).map(Stored::file).filter(|file| file.is_file())?;
        let flac = file.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("flac"));
        Some(AudioSource {
            url: format!("file:{}", file.display()),
            codec: if flac { Codec::Flac } else { Codec::Mp3 },
        })
    }

    pub fn stored(&self) -> impl Iterator<Item = &Stored> {
        self.0.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tactus_api::fake::track;

    #[test]
    fn finds_local_files_for_downloaded_tracks() {
        let folder = std::env::temp_dir().join(format!("tactus-offline-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("song.FLAC");
        std::fs::write(&file, b"fLaC").unwrap();
        let mut offline = Offline::default();
        offline.add(&track("1"), &file);
        offline.add(&track("2"), &folder.join("missing.mp3"));
        let source = offline.source("1").unwrap();
        assert_eq!((source.codec, source.url.starts_with("file:")), (Codec::Flac, true));
        assert!(offline.has("2") && offline.source("2").is_none());
        assert_eq!(offline.stored().count(), 2);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
