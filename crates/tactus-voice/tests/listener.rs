use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tactus_voice::{Config, Control, Heard, Listener, RATE, Source};

const CHUNK: usize = (RATE / 50) as usize;

fn folder() -> PathBuf {
    std::env::var("TACTUS_VOICE_TEST")
        .map_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/voice-test"), PathBuf::from)
}

fn samples(name: &str) -> Vec<i16> {
    let bytes = std::fs::read(folder().join(format!("{name}.wav"))).unwrap();
    let data = bytes.windows(4).position(|window| window == b"data").unwrap() + 8;
    bytes[data..].as_chunks::<2>().0.iter().map(|pair| i16::from_le_bytes(*pair)).collect()
}

struct Playback {
    samples: Vec<i16>,
    position: usize,
}

impl Source for Playback {
    fn read(&mut self, _timeout: Duration) -> Option<Vec<i16>> {
        std::thread::sleep(Duration::from_millis(20));
        let end = (self.position + CHUNK).min(self.samples.len());
        let mut chunk = self.samples[self.position..end].to_vec();
        self.position = end;
        chunk.resize(CHUNK, 0);
        Some(chunk)
    }
}

fn listener(recording: Option<&str>, model: &str, wake_word: Option<&str>) -> Listener {
    let lead = vec![0i16; RATE as usize / 4];
    let audio: Vec<i16> = lead.into_iter().chain(recording.map(samples).unwrap_or_default()).collect();
    let config = Config {
        runtime: folder().join("vosk-win64-0.3.45"),
        model: folder().join(model),
        microphone: String::new(),
        wake_word: wake_word.map(str::to_owned),
        silence: Duration::from_millis(800),
        wait: Duration::from_secs(2),
        limit: Duration::from_secs(8),
    };
    let listener = Listener::start_with(
        config,
        Box::new(move |_| Ok(Box::new(Playback { samples: audio.clone(), position: 0 }) as Box<dyn Source>)),
    );
    assert_eq!(wait(&listener, |heard| *heard == Heard::Ready, 30), [Heard::Ready]);
    listener
}

fn wait(listener: &Listener, done: impl Fn(&Heard) -> bool, seconds: u64) -> Vec<Heard> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut heard = Vec::new();
    while Instant::now() < deadline {
        for event in listener.take() {
            let finished = done(&event) || matches!(event, Heard::Failed(_));
            heard.push(event);
            if finished {
                return heard;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    heard
}

fn final_event(heard: &Heard) -> bool {
    matches!(heard, Heard::Text(_) | Heard::Nothing | Heard::Cancelled)
}

const RUSSIAN: &str = "vosk-model-small-ru-0.22";

#[test]
#[ignore]
fn a_tap_listens_until_the_speaker_pauses() {
    let listener = listener(Some("ru-next-1"), RUSSIAN, None);
    listener.send(Control::Hold);
    std::thread::sleep(Duration::from_millis(100));
    listener.send(Control::Tap);
    let heard = wait(&listener, final_event, 15);
    assert_eq!(heard, [Heard::Listening, Heard::Text("следующий трек".into())]);
}

#[test]
#[ignore]
fn holding_listens_until_release() {
    let listener = listener(Some("ru-volume-1"), RUSSIAN, None);
    listener.send(Control::Hold);
    std::thread::sleep(Duration::from_millis(3_500));
    listener.send(Control::Release);
    let heard = wait(&listener, final_event, 10);
    assert_eq!(heard, [Heard::Listening, Heard::Text("сделай громкость тридцать".into())]);
}

#[test]
#[ignore]
fn silence_gives_up_and_cancel_stops() {
    let listener = listener(None, RUSSIAN, None);
    listener.send(Control::Tap);
    let started = Instant::now();
    assert_eq!(wait(&listener, final_event, 10), [Heard::Listening, Heard::Nothing]);
    assert!(started.elapsed() < Duration::from_secs(4));
    listener.send(Control::Hold);
    std::thread::sleep(Duration::from_millis(300));
    listener.send(Control::Cancel);
    assert_eq!(wait(&listener, final_event, 10), [Heard::Listening, Heard::Cancelled]);
}

#[test]
#[ignore]
fn the_wake_word_starts_a_command() {
    let listener = listener(Some("ru-wake-next-1"), RUSSIAN, Some("тактус"));
    let heard = wait(&listener, final_event, 15);
    assert_eq!(heard, [Heard::Woke, Heard::Text("следующий трек".into())]);
}
