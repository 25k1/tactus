use crate::settings::data_dir;
use crate::update::hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tactus_api::error::{Error, Result};
use tactus_api::net;
use tactus_voice::{Config, Language, Phrases};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: Option<&'static str>,
}

pub const RUNTIME: Artifact = Artifact {
    name: "vosk-win64-0.3.45",
    url: "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip",
    size: 14_882_445,
    sha256: Some("f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b"),
};

const MODELS: [(&str, ModelSize, Artifact); 4] = [
    (
        "ru",
        ModelSize::Small,
        Artifact {
            name: "vosk-model-small-ru-0.22",
            url: "https://alphacephei.com/vosk/models/vosk-model-small-ru-0.22.zip",
            size: 46_236_750,
            sha256: Some("961d5ff98a17f4aa6de69864d0aa71fa5bac682301d2b5d17a3f24c5c99a46d4"),
        },
    ),
    (
        "en",
        ModelSize::Small,
        Artifact {
            name: "vosk-model-small-en-us-0.15",
            url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip",
            size: 41_205_931,
            sha256: Some("30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498"),
        },
    ),
    (
        "ru",
        ModelSize::Large,
        Artifact {
            name: "vosk-model-ru-0.42",
            url: "https://alphacephei.com/vosk/models/vosk-model-ru-0.42.zip",
            size: 1_937_602_113,
            sha256: None,
        },
    ),
    (
        "en",
        ModelSize::Large,
        Artifact {
            name: "vosk-model-en-us-0.22",
            url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.22.zip",
            size: 1_913_365_522,
            sha256: None,
        },
    ),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelSize {
    #[default]
    Small,
    Large,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicWhileListening {
    #[default]
    Duck,
    Pause,
    Keep,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub language: String,
    pub models: BTreeMap<String, ModelSize>,
    pub microphone: String,
    pub wake_word: bool,
    pub silence_ms: u32,
    pub wait_ms: u32,
    pub limit_ms: u32,
    pub music: MusicWhileListening,
    pub log: bool,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            language: String::new(),
            models: BTreeMap::new(),
            microphone: String::new(),
            wake_word: false,
            silence_ms: 800,
            wait_ms: 5_000,
            limit_ms: 10_000,
            music: MusicWhileListening::Duck,
            log: false,
        }
    }
}

impl VoiceSettings {
    pub fn language_for(&self, interface: &str) -> &'static str {
        match (self.language.as_str(), interface) {
            ("en", _) | ("", "en") => "en",
            _ => "ru",
        }
    }

    pub fn size_for(&self, language: &str) -> ModelSize {
        self.models.get(language).copied().unwrap_or_default()
    }

    pub fn listener(&self, language: &str) -> Config {
        Config {
            runtime: RUNTIME.path(),
            model: model(language, self.size_for(language)).path(),
            microphone: self.microphone.clone(),
            wake_word: self.wake_word.then(|| if language == "en" { "tactus" } else { "тактус" }.to_owned()),
            silence: Duration::from_millis(u64::from(self.silence_ms)),
            wait: Duration::from_millis(u64::from(self.wait_ms)),
            limit: Duration::from_millis(u64::from(self.limit_ms)),
        }
    }

    pub fn clamp(&mut self) {
        self.silence_ms = self.silence_ms.clamp(300, 3_000);
        self.wait_ms = self.wait_ms.clamp(1_000, 30_000);
        self.limit_ms = self.limit_ms.clamp(2_000, 60_000);
    }
}

impl Artifact {
    pub fn path(&self) -> PathBuf {
        folder().join(self.name)
    }

    pub fn installed(&self) -> bool {
        self.path().is_dir()
    }
}

pub fn folder() -> PathBuf {
    data_dir().join("voice")
}

pub fn model(language: &str, size: ModelSize) -> Artifact {
    MODELS
        .iter()
        .find(|(code, model_size, _)| *code == language && *model_size == size)
        .or_else(|| MODELS.iter().find(|(code, model_size, _)| *code == "ru" && *model_size == size))
        .map_or(MODELS[0].2, |(_, _, artifact)| *artifact)
}

pub fn ready(language: &str, size: ModelSize) -> bool {
    RUNTIME.installed() && model(language, size).installed()
}

pub fn phrases_file(language: &str) -> PathBuf {
    folder().join(format!("phrases.{language}.txt"))
}

pub fn phrases(language: &str) -> Phrases {
    let mut phrases = Phrases::builtin(Language::from_code(language).unwrap_or(Language::Russian));
    if let Ok(text) = std::fs::read_to_string(phrases_file(language)) {
        for error in phrases.extend(&text) {
            crate::log::warn(format_args!("voice phrases line {}: {}", error.line, error.message));
        }
    }
    phrases
}

fn verify(artifact: &Artifact, size: u64, digest: &str) -> Result<()> {
    let hash_ok = artifact.sha256.is_none_or(|expected| expected == digest);
    if size != artifact.size || !hash_ok {
        return Err(Error::Api(format!("{} did not download correctly", artifact.name)));
    }
    Ok(())
}

fn unpack(archive: &std::path::Path) -> Result<()> {
    let mut command = std::process::Command::new("tar");
    command.arg("-xf").arg(archive).arg("-C").arg(folder());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let status = command.status()?;
    if status.success() { Ok(()) } else { Err(Error::Api(format!("could not unpack {}", archive.display()))) }
}

pub fn install(artifact: &Artifact, cancel: &AtomicBool, progress: impl Fn(u64, u64)) -> Result<()> {
    if artifact.installed() {
        return Ok(());
    }
    std::fs::create_dir_all(folder())?;
    let archive = folder().join(format!("{}.zip.part", artifact.name));
    let result = (|| {
        let (size, digest) = net::block_on(async {
            let mut response = net::client().get(artifact.url).send().await?.error_for_status()?;
            let mut file = std::fs::File::create(&archive)?;
            let (mut hasher, mut done) = (Sha256::new(), 0u64);
            while let Some(chunk) = response.chunk().await? {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::Api("cancelled".into()));
                }
                file.write_all(&chunk)?;
                hasher.update(&chunk);
                done += chunk.len() as u64;
                progress(done, artifact.size);
            }
            file.flush()?;
            Ok::<_, Error>((done, hex(&hasher.finalize())))
        })?;
        verify(artifact, size, &digest)?;
        unpack(&archive)?;
        if artifact.installed() {
            Ok(())
        } else {
            Err(Error::Api(format!("{} is missing after unpacking", artifact.name)))
        }
    })();
    let _ = std::fs::remove_file(&archive);
    result
}

pub fn remove(artifact: &Artifact) -> Result<()> {
    match std::fs::remove_dir_all(artifact.path()) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_models_by_language_and_size() {
        assert_eq!(model("ru", ModelSize::Small).name, "vosk-model-small-ru-0.22");
        assert_eq!(model("en", ModelSize::Large).name, "vosk-model-en-us-0.22");
        assert_eq!(model("ka", ModelSize::Small).name, "vosk-model-small-ru-0.22");
        assert!(MODELS.iter().all(|(_, size, artifact)| (*size == ModelSize::Small) == artifact.sha256.is_some()));
    }

    #[test]
    fn follows_the_interface_language_unless_chosen() {
        let mut settings = VoiceSettings::default();
        assert_eq!(
            [settings.language_for("en"), settings.language_for("ru"), settings.language_for("ka")],
            ["en", "ru", "ru"]
        );
        settings.language = "en".into();
        assert_eq!(settings.language_for("ka"), "en");
        settings.silence_ms = 10;
        settings.clamp();
        assert_eq!(settings.silence_ms, 300);
    }

    #[test]
    fn rejects_wrong_downloads() {
        let artifact = model("ru", ModelSize::Small);
        assert!(verify(&artifact, artifact.size, artifact.sha256.unwrap()).is_ok());
        assert!(verify(&artifact, artifact.size, "00").is_err());
        assert!(verify(&artifact, 1, artifact.sha256.unwrap()).is_err());
        let large = model("ru", ModelSize::Large);
        assert!(verify(&large, large.size, "anything").is_ok());
    }

    #[test]
    #[ignore]
    fn installs_and_removes_the_runtime() {
        let cancel = AtomicBool::new(false);
        let last = std::sync::atomic::AtomicU64::new(0);
        install(&RUNTIME, &cancel, |done, _| last.store(done, Ordering::Relaxed)).unwrap();
        assert!(RUNTIME.path().join("libvosk.dll").is_file());
        assert_eq!(last.load(Ordering::Relaxed), RUNTIME.size);
        remove(&RUNTIME).unwrap();
        assert!(!RUNTIME.installed());
    }
}
