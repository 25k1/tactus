pub const RATE: u32 = 16_000;
const FRAME: usize = (RATE / 50) as usize;
const FLOOR_ADAPT: f32 = 0.02;
const SPEECH_RATIO: f32 = 3.0;
const MIN_SPEECH: f32 = 300.0;

pub struct Resampler {
    step: f64,
    position: f64,
    previous: f32,
}

impl Resampler {
    pub fn new(input_rate: u32) -> Self {
        Self { step: f64::from(input_rate.max(1)) / f64::from(RATE), position: 0.0, previous: 0.0 }
    }

    pub fn push(&mut self, mono: &[f32], output: &mut Vec<i16>) {
        for &sample in mono {
            while self.position < 1.0 {
                let value = self.previous + (sample - self.previous) * self.position as f32;
                output.push((value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16);
                self.position += self.step;
            }
            self.position -= 1.0;
            self.previous = sample;
        }
    }
}

pub fn mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    interleaved.chunks(channels).map(|frame| frame.iter().sum::<f32>() / frame.len() as f32).collect()
}

pub struct Activity {
    floor: f32,
    pending: Vec<i16>,
}

impl Default for Activity {
    fn default() -> Self {
        Self { floor: MIN_SPEECH / SPEECH_RATIO, pending: Vec::new() }
    }
}

impl Activity {
    pub fn speech(&mut self, samples: &[i16]) -> bool {
        self.pending.extend_from_slice(samples);
        let mut speech = false;
        while self.pending.len() >= FRAME {
            let frame: Vec<i16> = self.pending.drain(..FRAME).collect();
            let energy = (frame.iter().map(|sample| f32::from(*sample).powi(2)).sum::<f32>() / FRAME as f32).sqrt();
            let loud = energy > (self.floor * SPEECH_RATIO).max(MIN_SPEECH);
            if !loud {
                self.floor += (energy - self.floor) * FLOOR_ADAPT;
            }
            speech |= loud;
        }
        speech
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|index| (index as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * amplitude)
            .collect()
    }

    #[test]
    fn resamples_to_sixteen_kilohertz() {
        for rate in [8_000, 16_000, 44_100, 48_000] {
            let mut output = Vec::new();
            let mut resampler = Resampler::new(rate);
            for chunk in tone(rate, 1.0, 0.5).chunks(480) {
                resampler.push(chunk, &mut output);
            }
            assert!((output.len() as i64 - i64::from(RATE)).abs() <= 2, "{rate}: {}", output.len());
            assert!(output.iter().all(|sample| sample.unsigned_abs() <= 16_384), "{rate}");
        }
    }

    #[test]
    fn mixes_channels_down() {
        assert_eq!(mono(&[1.0, 0.0, 0.5, 0.5], 2), [0.5, 0.5]);
        assert_eq!(mono(&[0.25, 0.75], 1), [0.25, 0.75]);
    }

    #[test]
    fn tells_speech_from_steady_noise() {
        let to_pcm =
            |samples: Vec<f32>| -> Vec<i16> { samples.iter().map(|sample| (sample * 32_767.0) as i16).collect() };
        let mut activity = Activity::default();
        let noise = to_pcm(tone(RATE, 1.0, 0.004));
        assert!(!activity.speech(&noise));
        assert!(activity.speech(&to_pcm(tone(RATE, 0.2, 0.3))));
        assert!(!activity.speech(&noise[..3_200]));
    }
}
