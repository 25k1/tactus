use super::sonic::Sonic;
use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

const CHUNK_FRAMES: usize = 1024;
const NORMAL: f32 = 1.0;

#[derive(Clone, Debug)]
pub struct Speed(Arc<AtomicU32>);

impl Default for Speed {
    fn default() -> Self {
        Self::new(NORMAL)
    }
}

impl Speed {
    pub fn new(value: f32) -> Self {
        Self(Arc::new(AtomicU32::new(value.to_bits())))
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }
}

fn is_normal(speed: f32) -> bool {
    (speed - NORMAL).abs() < 0.01
}

pub struct Tempo<S> {
    inner: S,
    speed: Speed,
    position_ms: Arc<AtomicU64>,
    sonic: Option<Sonic>,
    channels: ChannelCount,
    rate: SampleRate,
    consumed: u64,
    input: Vec<Sample>,
    output: Vec<Sample>,
    cursor: usize,
    flushed: bool,
}

impl<S: Source> Tempo<S> {
    pub fn new(inner: S, speed: Speed, position_ms: Arc<AtomicU64>, start: Duration) -> Self {
        let (channels, rate) = (inner.channels(), inner.sample_rate());
        let mut tempo = Self {
            inner,
            speed,
            position_ms,
            sonic: None,
            channels,
            rate,
            consumed: 0,
            input: Vec::with_capacity(CHUNK_FRAMES * usize::from(channels.get())),
            output: Vec::new(),
            cursor: 0,
            flushed: false,
        };
        tempo.reset(start);
        tempo
    }

    fn reset(&mut self, position: Duration) {
        self.sonic = Sonic::new(self.rate.get(), self.channels.get());
        self.consumed = (position.as_secs_f64() * f64::from(self.rate.get())) as u64 * u64::from(self.channels.get());
        self.output.clear();
        self.cursor = 0;
        self.flushed = false;
        self.publish();
    }

    fn publish(&self) {
        let frames = self.consumed / u64::from(self.channels.get());
        self.position_ms.store(frames * 1000 / u64::from(self.rate.get()), Ordering::Relaxed);
    }

    fn take_input(&mut self) -> Option<Sample> {
        let sample = self.inner.next()?;
        self.consumed += 1;
        if self.consumed.is_multiple_of(u64::from(self.channels.get()) * 256) {
            self.publish();
        }
        Some(sample)
    }

    fn drain(&mut self) -> bool {
        self.cursor = 0;
        self.sonic.as_mut().is_some_and(|sonic| sonic.read(&mut self.output))
    }

    fn refill(&mut self, speed: f32) -> bool {
        let channels = usize::from(self.channels.get());
        loop {
            self.input.clear();
            while self.input.len() < CHUNK_FRAMES * channels
                && let Some(sample) = self.take_input()
            {
                self.input.push(sample);
            }
            let Some(sonic) = self.sonic.as_mut() else {
                return false;
            };
            sonic.set_speed(speed);
            if !self.input.is_empty() {
                sonic.write(&self.input);
            } else if !self.flushed {
                sonic.flush();
                self.flushed = true;
            }
            if self.drain() {
                return true;
            }
            if self.input.is_empty() {
                return false;
            }
        }
    }
}

impl<S: Source> Iterator for Tempo<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if let Some(sample) = self.output.get(self.cursor) {
            self.cursor += 1;
            return Some(*sample);
        }
        let speed = self.speed.get();
        let normal = is_normal(speed) || self.sonic.is_none();
        if if normal { self.drain() } else { self.refill(speed) } {
            return self.next();
        }
        let sample = if normal { self.take_input() } else { None };
        if sample.is_none() {
            self.publish();
        }
        sample
    }
}

impl<S: Source> Source for Tempo<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(position)?;
        self.reset(position);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::source::SineWave;

    fn played(speed: f32, seconds: f32) -> (usize, u64) {
        let position = Arc::new(AtomicU64::new(0));
        let source = SineWave::new(440.0).take_duration(Duration::from_secs_f32(seconds));
        let tempo = Tempo::new(source, Speed::new(speed), position.clone(), Duration::ZERO);
        let count = tempo.count();
        (count, position.load(Ordering::Relaxed))
    }

    #[test]
    fn normal_speed_passes_audio_through() {
        let (samples, position) = played(1.0, 1.0);
        assert_eq!(samples, 48_000);
        assert_eq!(position, 1000);
    }

    #[test]
    fn double_speed_halves_the_output_and_keeps_media_time() {
        let (samples, position) = played(2.0, 1.0);
        assert!((22_000..=26_000).contains(&samples), "{samples}");
        assert_eq!(position, 1000);
    }

    #[test]
    fn slow_speed_stretches_the_output() {
        let (samples, _) = played(0.5, 1.0);
        assert!((94_000..=98_000).contains(&samples), "{samples}");
    }
}
