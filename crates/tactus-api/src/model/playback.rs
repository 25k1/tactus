use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Lossless,
    High,
    Low,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Lossless, Quality::High, Quality::Low];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Mp3,
    Flac,
    Aac,
}

impl Codec {
    pub fn parse(name: &str) -> Codec {
        match name {
            "flac" | "flac-mp4" => Codec::Flac,
            "aac" | "he-aac" | "aac-mp4" | "he-aac-mp4" => Codec::Aac,
            _ => Codec::Mp3,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Codec::Mp3 => "mp3",
            Codec::Flac => "flac",
            Codec::Aac => "m4a",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioSource {
    pub url: String,
    pub codec: Codec,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LyricLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    pub writers: Vec<String>,
}

impl Lyrics {
    pub fn synced(&self) -> bool {
        self.lines.iter().any(|line| line.time_ms.is_some())
    }

    pub fn line_at(&self, position_ms: u64) -> Option<usize> {
        self.lines.iter().rposition(|line| line.time_ms.is_some_and(|time| time <= position_ms))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackInfo {
    pub track: Track,
    pub artists: Vec<String>,
    pub album_type: String,
    pub release_date: String,
    pub labels: Vec<String>,
    pub lyrics: bool,
    pub synced_lyrics: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaveBatch {
    pub session: String,
    pub batch: String,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WaveEvent {
    RadioStarted,
    TrackStarted { track_id: String },
    TrackFinished { track_id: String, seconds: f64 },
    Skip { track_id: String, seconds: f64 },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteQueue {
    pub id: String,
    pub track_ids: Vec<String>,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Play {
    pub track: Track,
    pub played_seconds: u32,
    pub from: String,
}
