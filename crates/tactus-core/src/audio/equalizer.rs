use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};
use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

pub const BANDS: usize = 10;
pub const FREQUENCIES: [f32; BANDS] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
pub const MAX_GAIN_DB: f32 = 12.0;
const Q: f32 = 1.41;

pub type Gains = [f32; BANDS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Flat,
    Bass,
    Treble,
    Vocal,
    Speech,
    Rock,
    Pop,
    Jazz,
    Classical,
    Electronic,
}

impl Preset {
    pub const ALL: [Preset; 10] = [
        Preset::Flat,
        Preset::Bass,
        Preset::Treble,
        Preset::Vocal,
        Preset::Speech,
        Preset::Rock,
        Preset::Pop,
        Preset::Jazz,
        Preset::Classical,
        Preset::Electronic,
    ];

    pub fn gains(self) -> Gains {
        match self {
            Preset::Flat => [0.0; BANDS],
            Preset::Bass => [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            Preset::Treble => [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
            Preset::Vocal => [-2.0, -2.0, -1.0, 1.0, 3.0, 3.0, 2.0, 1.0, 0.0, -1.0],
            Preset::Speech => [-4.0, -3.0, -1.0, 1.0, 3.0, 4.0, 3.0, 2.0, 0.0, -2.0],
            Preset::Rock => [4.0, 3.0, 2.0, 0.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
            Preset::Pop => [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0],
            Preset::Jazz => [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0],
            Preset::Classical => [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
            Preset::Electronic => [5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 1.0, 4.0, 5.0],
        }
    }

    pub fn matching(gains: &Gains) -> Option<Preset> {
        Self::ALL.into_iter().find(|preset| preset.gains() == *gains)
    }
}

#[derive(Debug, Default)]
pub struct Shared {
    gains: [AtomicU32; BANDS],
    version: AtomicU64,
}

impl Shared {
    pub fn set(&self, gains: &Gains) {
        for (slot, gain) in self.gains.iter().zip(gains) {
            slot.store(gain.clamp(-MAX_GAIN_DB, MAX_GAIN_DB).to_bits(), Ordering::Relaxed);
        }
        self.version.fetch_add(1, Ordering::Release);
    }

    fn gains(&self) -> Gains {
        std::array::from_fn(|band| f32::from_bits(self.gains[band].load(Ordering::Relaxed)))
    }

    fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    fn peaking(frequency: f32, gain_db: f32, rate: f32) -> Option<Biquad> {
        if gain_db.abs() < 0.05 || frequency >= rate * 0.45 {
            return None;
        }
        let amplitude = 10f32.powf(gain_db / 40.0);
        let omega = 2.0 * PI * frequency / rate;
        let alpha = omega.sin() / (2.0 * Q);
        let cos = omega.cos();
        let a0 = 1.0 + alpha / amplitude;
        Some(Biquad {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / amplitude) / a0,
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Memory {
    z1: f32,
    z2: f32,
}

impl Memory {
    fn process(&mut self, filter: &Biquad, input: f32) -> f32 {
        let output = filter.b0 * input + self.z1;
        self.z1 = filter.b1 * input - filter.a1 * output + self.z2;
        self.z2 = filter.b2 * input - filter.a2 * output;
        output
    }
}

impl Biquad {
    fn magnitude(&self, omega: f32) -> f32 {
        let (cos1, sin1, cos2, sin2) = (omega.cos(), omega.sin(), (2.0 * omega).cos(), (2.0 * omega).sin());
        let numerator = (self.b0 + self.b1 * cos1 + self.b2 * cos2).hypot(self.b1 * sin1 + self.b2 * sin2);
        let denominator = (1.0 + self.a1 * cos1 + self.a2 * cos2).hypot(self.a1 * sin1 + self.a2 * sin2);
        numerator / denominator
    }
}

fn preamp(filters: &[Biquad], rate: f32) -> f32 {
    let peak = (0..=96)
        .map(|step| 20.0 * 1000f32.powf(step as f32 / 96.0))
        .filter(|frequency| *frequency < rate / 2.0)
        .map(|frequency| filters.iter().map(|filter| filter.magnitude(2.0 * PI * frequency / rate)).product::<f32>())
        .fold(1.0f32, f32::max);
    1.0 / peak
}

pub struct Equalizer<S> {
    inner: S,
    shared: Arc<Shared>,
    version: u64,
    filters: Vec<Biquad>,
    memory: Vec<Memory>,
    preamp: f32,
    channels: usize,
    channel: usize,
}

impl<S: Source> Equalizer<S> {
    pub fn new(inner: S, shared: Arc<Shared>) -> Self {
        let channels = usize::from(inner.channels().get());
        let mut equalizer = Self {
            inner,
            shared,
            version: u64::MAX,
            filters: Vec::new(),
            memory: Vec::new(),
            preamp: 1.0,
            channels,
            channel: 0,
        };
        equalizer.refresh();
        equalizer
    }

    fn refresh(&mut self) {
        let version = self.shared.version();
        if version == self.version {
            return;
        }
        self.version = version;
        let gains = self.shared.gains();
        let rate = self.inner.sample_rate().get() as f32;
        self.filters = FREQUENCIES
            .iter()
            .zip(gains)
            .filter_map(|(frequency, gain)| Biquad::peaking(*frequency, gain, rate))
            .collect();
        self.memory = vec![Memory::default(); self.filters.len() * self.channels];
        self.preamp = preamp(&self.filters, rate);
    }
}

impl<S: Source> Iterator for Equalizer<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.channel == 0 {
            self.refresh();
        }
        let sample = self.inner.next()?;
        let channel = self.channel;
        self.channel = (self.channel + 1) % self.channels;
        if self.filters.is_empty() {
            return Some(sample);
        }
        let bands = self.filters.len();
        let memory = &mut self.memory[channel * bands..(channel + 1) * bands];
        let output = self
            .filters
            .iter()
            .zip(memory)
            .fold(sample * self.preamp, |value, (filter, memory)| memory.process(filter, value));
        Some(output)
    }
}

impl<S: Source> Source for Equalizer<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(position)?;
        self.memory.iter_mut().for_each(|memory| *memory = Memory::default());
        self.channel = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::source::SineWave;

    fn rms(gains: Gains, frequency: f32) -> f32 {
        let shared = Arc::new(Shared::default());
        shared.set(&gains);
        let source = SineWave::new(frequency).take_duration(Duration::from_millis(500));
        let samples: Vec<f32> = Equalizer::new(source, shared).skip(4_800).collect();
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn flat_passes_audio_through() {
        let source = SineWave::new(440.0).take_duration(Duration::from_millis(10));
        let expected: Vec<f32> = source.clone().collect();
        let output: Vec<f32> = Equalizer::new(source, Arc::new(Shared::default())).collect();
        assert_eq!(output, expected);
    }

    #[test]
    fn cutting_a_band_attenuates_its_frequency() {
        let flat = rms([0.0; BANDS], 1000.0);
        let mut cut = [0.0; BANDS];
        cut[5] = -12.0;
        assert!(rms(cut, 1000.0) < flat * 0.4);
        assert!((rms(cut, 31.0) - rms([0.0; BANDS], 31.0)).abs() < 0.05);
    }

    #[test]
    fn boosting_never_clips_a_full_scale_signal() {
        let samples_peak = {
            let shared = Arc::new(Shared::default());
            shared.set(&Preset::Bass.gains());
            Equalizer::new(SineWave::new(62.0).take_duration(Duration::from_millis(500)), shared)
                .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
        };
        assert!(samples_peak <= 1.05, "{samples_peak}");
    }

    #[test]
    fn presets_round_trip() {
        for preset in Preset::ALL {
            assert_eq!(Preset::matching(&preset.gains()), Some(preset));
        }
        assert_eq!(Preset::matching(&[1.0; BANDS]), None);
    }
}
