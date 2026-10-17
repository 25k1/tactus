mod buffer;
pub mod equalizer;
#[cfg(test)]
pub mod mock;
mod output;
#[allow(unsafe_code)]
mod sonic;
mod tempo;

pub use buffer::Buffer;
use buffer::Reader;
pub use equalizer::{BANDS, Gains, Preset};
pub use output::RodioOutput;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, Source};
use std::sync::Arc;
use std::time::Duration;
use tactus_api::error::{Error, Result};
use tactus_api::{AudioSource, Codec, Loudness};

const TARGET_LOUDNESS: f32 = -14.0;
const PEAK_CEILING: f32 = -1.0;
const MAX_BOOST_DB: f32 = 10.0;

pub type Track = Box<dyn Source + Send>;

pub struct Loaded {
    source: Track,
    replay: Option<(Arc<Buffer>, Codec)>,
}

impl Loaded {
    pub fn from_source(source: Track) -> Self {
        Self { source, replay: None }
    }

    fn reload((buffer, codec): (Arc<Buffer>, Codec)) -> Result<Loaded> {
        Ok(Loaded { source: Box::new(decode(&buffer, codec)?), replay: Some((buffer, codec)) })
    }
}

pub type Loader = Arc<dyn Fn(&AudioSource) -> Result<Loaded> + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    pub playing: bool,
    pub at: Duration,
    pub gain: f32,
    pub fade: Duration,
}

impl Start {
    pub fn new(playing: bool, at: Duration, gain: f32) -> Self {
        Self { playing, at, gain, fade: Duration::ZERO }
    }
}

pub trait Output {
    fn start(&mut self, track: Loaded, start: Start) -> Result<()>;
    fn restart(&mut self, start: Start) -> Result<()>;
    fn stop(&mut self);
    fn is_loaded(&self) -> bool;
    fn set_playing(&mut self, playing: bool);
    fn position(&self) -> Duration;
    fn seek(&mut self, position: Duration);
    fn finished(&self) -> bool;
    fn set_volume(&mut self, volume: u8);
    fn set_speed(&mut self, speed: f32);
    fn set_equalizer(&mut self, gains: &Gains);
    fn set_device(&mut self, device: &str) -> Result<()>;
    fn fade_out(&mut self, duration: Duration);
    fn tick(&mut self);
}

fn decode(buffer: &Arc<Buffer>, codec: Codec) -> Result<Decoder<Reader>> {
    let mut builder = Decoder::builder().with_data(buffer.reader()).with_hint(codec.extension()).with_seekable(true);
    if let Some(total) = buffer.total() {
        builder = builder.with_byte_len(total);
    }
    builder.build().map_err(|error| Error::Audio(error.to_string()))
}

pub fn load(source: &AudioSource) -> Result<Loaded> {
    let buffer = match source.url.strip_prefix("file:") {
        Some(path) => Buffer::complete(std::fs::read(path)?),
        None => Buffer::fetch(source.url.clone()),
    };
    let decoder = decode(&buffer, source.codec)?;
    Ok(Loaded { source: Box::new(decoder), replay: Some((buffer, source.codec)) })
}

pub fn loader() -> Loader {
    Arc::new(load)
}

fn names<D: DeviceTrait, E>(devices: std::result::Result<impl Iterator<Item = D>, E>) -> Vec<String> {
    devices
        .map(|devices| devices.filter_map(|device| Some(device.description().ok()?.name().to_owned())).collect())
        .unwrap_or_default()
}

pub fn devices() -> Vec<String> {
    names(rodio::cpal::default_host().output_devices())
}

pub fn input_devices() -> Vec<String> {
    names(rodio::cpal::default_host().input_devices())
}

pub fn normalization_gain(loudness: Option<Loudness>) -> f32 {
    let Some(loudness) = loudness else {
        return 1.0;
    };
    let gain_db = (TARGET_LOUDNESS - loudness.integrated).min(PEAK_CEILING - loudness.peak).clamp(-20.0, MAX_BOOST_DB);
    10f32.powf(gain_db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decibels(gain: f32) -> f32 {
        20.0 * gain.log10()
    }

    #[test]
    fn normalizes_loud_and_quiet_tracks() {
        assert_eq!(normalization_gain(None), 1.0);
        let loud = normalization_gain(Some(Loudness { integrated: -8.0, peak: 0.5 }));
        assert!((decibels(loud) + 6.0).abs() < 0.01);
        let quiet = normalization_gain(Some(Loudness { integrated: -24.0, peak: -12.0 }));
        assert!((decibels(quiet) - 10.0).abs() < 0.01);
        let peaky = normalization_gain(Some(Loudness { integrated: -20.0, peak: -3.0 }));
        assert!((decibels(peaky) - 2.0).abs() < 0.01);
    }
}
