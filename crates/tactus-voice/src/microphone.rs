use crate::audio::{Resampler, mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

pub struct Microphone {
    _stream: cpal::Stream,
    samples: Receiver<Vec<i16>>,
}

fn device(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    let named = (!name.is_empty())
        .then(|| host.input_devices().ok()?.find(|device| device.description().is_ok_and(|found| found.name() == name)))
        .flatten();
    named.or_else(|| host.default_input_device())
}

fn build<T: SizedSample>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    sender: Sender<Vec<i16>>,
) -> Result<cpal::Stream, String>
where
    f32: FromSample<T>,
{
    let (channels, mut resampler) = (usize::from(config.channels), Resampler::new(config.sample_rate));
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let floats: Vec<f32> = data.iter().map(|sample| f32::from_sample_(*sample)).collect();
                let mut output = Vec::with_capacity(floats.len() / channels.max(1));
                resampler.push(&mono(&floats, channels), &mut output);
                let _ = sender.send(output);
            },
            |_| {},
            None,
        )
        .map_err(|error| error.to_string())
}

pub trait Source {
    fn read(&mut self, timeout: Duration) -> Option<Vec<i16>>;
}

impl Source for Microphone {
    fn read(&mut self, timeout: Duration) -> Option<Vec<i16>> {
        self.samples.recv_timeout(timeout).ok()
    }
}

impl Microphone {
    pub fn open(name: &str) -> Result<Microphone, String> {
        let device = device(name).ok_or("no microphone was found")?;
        let supported = device.default_input_config().map_err(|error| error.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let (sender, samples) = mpsc::channel();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, sender),
            SampleFormat::I16 => build::<i16>(&device, config, sender),
            SampleFormat::U16 => build::<u16>(&device, config, sender),
            SampleFormat::I32 => build::<i32>(&device, config, sender),
            other => Err(format!("unsupported microphone format {other}")),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Microphone { _stream: stream, samples })
    }
}
