use crate::announce::Announcement;
use crate::audio::{BANDS, Gains};
use crate::hotkey::GlobalAction;
use crate::player::queue::Repeat;
use crate::voice::VoiceSettings;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tactus_api::Quality;
use tactus_api::error::Result;

pub const DEFAULT_REMOTE_PORT: u16 = 9420;
pub const MAX_CROSSFADE: u8 = 12;
pub const DEFAULT_TRACK_FORMAT: &str = "{title} — {artist}";
pub const DEFAULT_DOWNLOAD_TEMPLATE: &str = "{artist}/{album}/{number} {title}";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Column {
    Title,
    Artist,
    Details,
    Duration,
}

impl Column {
    pub const ALL: [Column; 4] = [Column::Title, Column::Artist, Column::Details, Column::Duration];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: Option<String>,
    pub theme: Theme,
    pub volume: u8,
    pub speed: f32,
    pub spoken_speed: f32,
    pub normalize: bool,
    pub crossfade: u8,
    pub equalizer: bool,
    pub equalizer_gains: Gains,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub stream_quality: Quality,
    pub output_device: String,
    pub continue_with_wave: bool,
    pub sync_queue: bool,
    pub tray: bool,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub autostart: bool,
    pub resume_session: bool,
    pub autoplay: bool,
    pub media_controls: bool,
    pub notifications: bool,
    pub check_updates: bool,
    pub columns: Vec<Column>,
    pub compact_player: bool,
    pub hide_played: bool,
    pub episode_notifications: bool,
    pub ynison: bool,
    pub device_id: String,
    pub device_name: String,
    pub voice: VoiceSettings,
    pub app_keys: BTreeMap<String, String>,
    pub announcements: BTreeSet<Announcement>,
    pub track_format: String,
    pub sound_cues: bool,
    pub global_hotkeys: bool,
    pub hotkeys: BTreeMap<GlobalAction, String>,
    pub download_dir: String,
    pub download_quality: Quality,
    pub download_template: String,
    pub embed_cover: bool,
    pub remote_enabled: bool,
    pub remote_port: u16,
    pub remote_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            theme: Theme::System,
            volume: 100,
            speed: 1.0,
            spoken_speed: 1.0,
            normalize: true,
            crossfade: 0,
            equalizer: false,
            equalizer_gains: [0.0; BANDS],
            repeat: Repeat::Off,
            shuffle: false,
            stream_quality: Quality::High,
            output_device: String::new(),
            continue_with_wave: true,
            sync_queue: false,
            tray: true,
            minimize_to_tray: true,
            start_minimized: false,
            autostart: false,
            resume_session: true,
            autoplay: false,
            media_controls: true,
            notifications: false,
            check_updates: true,
            columns: Column::ALL.to_vec(),
            compact_player: false,
            hide_played: false,
            episode_notifications: false,
            ynison: false,
            device_id: String::new(),
            device_name: String::new(),
            voice: VoiceSettings::default(),
            app_keys: BTreeMap::new(),
            announcements: Announcement::defaults(),
            track_format: DEFAULT_TRACK_FORMAT.to_owned(),
            sound_cues: true,
            global_hotkeys: true,
            hotkeys: GlobalAction::defaults(),
            download_dir: String::new(),
            download_quality: Quality::Lossless,
            download_template: DEFAULT_DOWNLOAD_TEMPLATE.to_owned(),
            embed_cover: true,
            remote_enabled: false,
            remote_port: DEFAULT_REMOTE_PORT,
            remote_token: String::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut settings: Settings = read_json("settings.json").unwrap_or_default();
        settings.language = settings.language.and_then(|language| normalize_language(&language));
        settings.volume = settings.volume.min(100);
        settings.speed = settings.speed.clamp(0.5, 2.0);
        settings.spoken_speed = settings.spoken_speed.clamp(0.5, 2.0);
        settings.voice.clamp();
        settings.crossfade = settings.crossfade.min(MAX_CROSSFADE);
        settings
    }

    pub fn save(&self) -> Result<()> {
        write_json("settings.json", self)
    }

    pub fn speed_for(&self, spoken: bool) -> f32 {
        if spoken { self.spoken_speed } else { self.speed }
    }

    pub fn active_equalizer(&self) -> Gains {
        if self.equalizer { self.equalizer_gains } else { [0.0; BANDS] }
    }

    pub fn download_folder(&self) -> PathBuf {
        match self.download_dir.trim() {
            "" => app_dir().join("Downloads"),
            folder => app_dir().join(folder),
        }
    }
}

pub fn normalize_language(language: &str) -> Option<String> {
    let language = language.trim().to_ascii_lowercase();
    (!language.is_empty() && language.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .then_some(language)
}

pub fn app_dir() -> &'static Path {
    static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    });
    &DIR
}

pub fn data_dir() -> PathBuf {
    app_dir().join("data")
}

pub fn portable_path(path: &str) -> String {
    Path::new(path).strip_prefix(app_dir()).map_or_else(|_| path.to_owned(), |relative| relative.display().to_string())
}

fn read_text(name: &str) -> Option<String> {
    std::fs::read_to_string(data_dir().join(name)).ok()
}

fn write_text(name: &str, contents: &str) -> Result<()> {
    let directory = data_dir();
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(name);
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, contents)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(name: &str) -> Option<T> {
    serde_json::from_str(&read_text(name)?).ok()
}

pub fn write_json<T: Serialize>(name: &str, value: &T) -> Result<()> {
    write_text(name, &serde_json::to_string_pretty(value)?)
}

pub fn load_token() -> Option<String> {
    read_text("token").map(|token| token.trim().to_owned()).filter(|token| !token.is_empty())
}

pub fn save_token(token: &str) -> Result<()> {
    write_text("token", token)
}

pub fn delete_token() -> Result<()> {
    match std::fs::remove_file(data_dir().join("token")) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_use_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"volume": 40, "unknown": 1}"#).unwrap();
        assert_eq!(settings.volume, 40);
        assert_eq!(settings.remote_port, DEFAULT_REMOTE_PORT);
        assert_eq!(settings.hotkeys, GlobalAction::defaults());
    }

    #[test]
    fn settings_round_trip() {
        let settings = Settings::default();
        let text = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), settings);
    }

    #[test]
    fn keeps_downloads_and_data_next_to_the_executable() {
        let settings = Settings::default();
        assert_eq!(settings.download_folder(), app_dir().join("Downloads"));
        assert_eq!(data_dir(), app_dir().join("data"));
        let inside = app_dir().join("Music").join("Rock");
        let relative = portable_path(&inside.to_string_lossy());
        assert_eq!(Path::new(&relative), Path::new("Music").join("Rock"));
        let custom = Settings { download_dir: relative, ..Settings::default() };
        assert_eq!(custom.download_folder(), inside);
        let outside = std::env::temp_dir().to_string_lossy().into_owned();
        assert_eq!(portable_path(&outside), outside);
        assert_eq!(
            Settings { download_dir: outside.clone(), ..Settings::default() }.download_folder(),
            Path::new(&outside)
        );
    }

    #[test]
    fn normalizes_languages() {
        assert_eq!(normalize_language(" RU \n").as_deref(), Some("ru"));
        assert_eq!(normalize_language("../x"), None);
    }
}
