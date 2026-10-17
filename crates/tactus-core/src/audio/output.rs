use super::equalizer::{self, Equalizer, Gains};
use super::tempo::{Speed, Tempo};
use super::{Buffer, Loaded, Output, Start};
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tactus_api::Codec;
use tactus_api::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    Keep,
    Stop,
    Pause,
}

#[derive(Clone, Copy, Debug)]
struct Ramp {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    then: Then,
}

impl Ramp {
    fn new(from: f32, to: f32, duration: Duration, then: Then) -> Self {
        Self { from, to, started: Instant::now(), duration, then }
    }

    fn progress(&self) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (self.started.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }

    fn level(&self) -> f32 {
        self.from + (self.to - self.from) * self.progress()
    }

    fn done(&self) -> bool {
        self.progress() >= 1.0
    }
}

struct Voice {
    sink: Player,
    position_ms: Arc<AtomicU64>,
    gain: f32,
    replay: Option<(Arc<Buffer>, Codec)>,
    ramp: Option<Ramp>,
}

impl Voice {
    fn level(&self) -> f32 {
        self.ramp.as_ref().map_or(1.0, Ramp::level)
    }

    fn apply(&self, volume: f32) {
        self.sink.set_volume(volume * self.gain * self.level());
    }
}

pub struct RodioOutput {
    stream: Option<MixerDeviceSink>,
    device: String,
    current: Option<Voice>,
    outgoing: Vec<Voice>,
    volume: f32,
    speed: Speed,
    equalizer: Arc<equalizer::Shared>,
}

fn open_stream(device: &str) -> Result<MixerDeviceSink> {
    let named = (!device.is_empty())
        .then(|| {
            rodio::cpal::default_host()
                .output_devices()
                .ok()?
                .find(|candidate| candidate.description().is_ok_and(|found| found.name() == device))
        })
        .flatten();
    let builder = match named {
        Some(device) => DeviceSinkBuilder::from_device(device),
        None => DeviceSinkBuilder::from_default_device(),
    };
    let mut stream =
        builder.and_then(|builder| builder.open_sink_or_fallback()).map_err(|error| Error::Audio(error.to_string()))?;
    stream.log_on_drop(false);
    Ok(stream)
}

impl RodioOutput {
    pub fn new(device: &str, volume: u8, speed: f32, gains: &Gains) -> Self {
        let equalizer = Arc::new(equalizer::Shared::default());
        equalizer.set(gains);
        Self {
            stream: None,
            device: device.to_owned(),
            current: None,
            outgoing: Vec::new(),
            volume: f32::from(volume) / 100.0,
            speed: Speed::new(speed),
            equalizer,
        }
    }

    fn voice(&mut self, track: Loaded, start: Start) -> Result<Voice> {
        if self.stream.is_none() {
            self.stream = Some(open_stream(&self.device)?);
        }
        let stream = self.stream.as_ref().ok_or_else(|| Error::Audio("no audio output".into()))?;
        let Loaded { mut source, replay } = track;
        let at = if !start.at.is_zero() && source.try_seek(start.at).is_ok() { start.at } else { Duration::ZERO };
        let position_ms = Arc::new(AtomicU64::new(at.as_millis() as u64));
        let sink = Player::connect_new(stream.mixer());
        sink.pause();
        let tempo = Tempo::new(source, self.speed.clone(), position_ms.clone(), at);
        sink.append(Equalizer::new(tempo, self.equalizer.clone()));
        let ramp = (!start.fade.is_zero()).then(|| Ramp::new(0.0, 1.0, start.fade, Then::Keep));
        let voice = Voice { sink, position_ms, gain: start.gain, replay, ramp };
        voice.apply(self.volume);
        if start.playing {
            voice.sink.play();
        }
        Ok(voice)
    }

    fn retire(&mut self, fade: Duration) {
        let Some(mut voice) = self.current.take() else {
            return;
        };
        if fade.is_zero() {
            voice.sink.stop();
        } else {
            voice.ramp = Some(Ramp::new(voice.level(), 0.0, fade, Then::Stop));
            self.outgoing.push(voice);
        }
    }
}

impl Output for RodioOutput {
    fn start(&mut self, track: Loaded, start: Start) -> Result<()> {
        let voice = self.voice(track, start)?;
        self.retire(start.fade);
        self.current = Some(voice);
        Ok(())
    }

    fn restart(&mut self, start: Start) -> Result<()> {
        let replay = self
            .current
            .as_ref()
            .and_then(|voice| voice.replay.clone())
            .ok_or_else(|| Error::Audio("nothing is loaded".into()))?;
        self.start(Loaded::reload(replay)?, start)
    }

    fn stop(&mut self) {
        self.retire(Duration::ZERO);
        self.outgoing.drain(..).for_each(|voice| voice.sink.stop());
    }

    fn is_loaded(&self) -> bool {
        self.current.is_some()
    }

    fn set_playing(&mut self, playing: bool) {
        if let Some(voice) = &mut self.current {
            if playing {
                voice.ramp = None;
                voice.apply(self.volume);
                voice.sink.play();
            } else {
                voice.sink.pause();
            }
        }
    }

    fn position(&self) -> Duration {
        self.current
            .as_ref()
            .map_or(Duration::ZERO, |voice| Duration::from_millis(voice.position_ms.load(Ordering::Relaxed)))
    }

    fn seek(&mut self, position: Duration) {
        if let Some(voice) = &self.current
            && voice.sink.try_seek(position).is_ok()
        {
            voice.position_ms.store(position.as_millis() as u64, Ordering::Relaxed);
        }
    }

    fn finished(&self) -> bool {
        self.current.as_ref().is_some_and(|voice| voice.sink.empty())
    }

    fn set_volume(&mut self, volume: u8) {
        self.volume = f32::from(volume) / 100.0;
        self.current.iter().chain(&self.outgoing).for_each(|voice| voice.apply(self.volume));
    }

    fn set_speed(&mut self, speed: f32) {
        self.speed.set(speed);
    }

    fn set_equalizer(&mut self, gains: &Gains) {
        self.equalizer.set(gains);
    }

    fn set_device(&mut self, device: &str) -> Result<()> {
        if self.device == device {
            return Ok(());
        }
        let resume = self.current.as_ref().and_then(|voice| {
            let replay = voice.replay.clone()?;
            Some((replay, Start::new(!voice.sink.is_paused(), self.position(), voice.gain)))
        });
        self.stop();
        self.stream = None;
        self.device = device.to_owned();
        if let Some((replay, start)) = resume {
            self.start(Loaded::reload(replay)?, start)?;
        }
        Ok(())
    }

    fn fade_out(&mut self, duration: Duration) {
        if let Some(voice) = &mut self.current {
            voice.ramp = Some(Ramp::new(voice.level(), 0.0, duration, Then::Pause));
        }
    }

    fn tick(&mut self) {
        let volume = self.volume;
        self.outgoing.retain(|voice| {
            voice.apply(volume);
            let done = voice.ramp.is_none_or(|ramp| ramp.done());
            if done {
                voice.sink.stop();
            }
            !done
        });
        if let Some(voice) = &mut self.current {
            voice.apply(volume);
            if let Some(ramp) = voice.ramp.filter(Ramp::done) {
                if ramp.then == Then::Pause {
                    voice.sink.pause();
                }
                voice.ramp = None;
                voice.apply(volume);
            }
        }
    }
}
