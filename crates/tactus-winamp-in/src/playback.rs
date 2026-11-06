use crate::host::{Host, Output};
use crate::{library, prefetch};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tactus_api::Track;
use tactus_api::error::{Error, ErrorKind, Result};
use tactus_api::sync::Guard;
use tactus_stream::{Buffer, Cancel, Decoder, Reader, Source};
use tactus_winamp::sdk::COMMAND_STOP;
use tactus_winamp::{log, text, tracks};

const BLOCK: usize = 576;
const POLL: Duration = Duration::from_millis(10);
const RETRY_DELAYS: [Duration; 3] = [Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10)];
const SKIP_LIMIT: u32 = 3;

#[derive(Default)]
struct Control {
    killed: AtomicBool,
    ready: AtomicBool,
    seek: AtomicI64,
    cancel: Mutex<Option<Cancel>>,
    opened: Mutex<bool>,
}

impl Control {
    fn new() -> Self {
        Self { seek: AtomicI64::new(-1), ..Self::default() }
    }

    fn killed(&self) -> bool {
        self.killed.load(Ordering::Acquire)
    }

    fn kill(&self) {
        self.killed.store(true, Ordering::Release);
        if let Some(cancel) = self.cancel.guard().as_ref() {
            cancel.cancel();
        }
    }

    fn watch(&self, cancel: Cancel) -> bool {
        let mut slot = self.cancel.guard();
        if self.killed() {
            cancel.cancel();
            return false;
        }
        *slot = Some(cancel);
        true
    }

    fn seek_pending(&self) -> bool {
        self.seek.load(Ordering::Acquire) >= 0
    }

    fn take_seek(&self) -> Option<u64> {
        u64::try_from(self.seek.swap(-1, Ordering::AcqRel)).ok()
    }

    fn rest(&self, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until && !self.killed() {
            std::thread::sleep(POLL);
        }
    }
}

struct Session {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
    paused: bool,
}

enum Pumped {
    Finished,
    Killed,
    Broken,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static CURRENT: Mutex<Option<Track>> = Mutex::new(None);
static FAILURES: AtomicU32 = AtomicU32::new(0);

pub fn play(id: String) {
    stop();
    prefetch::arm();
    let control = Arc::new(Control::new());
    let worker = control.clone();
    let thread = std::thread::Builder::new().name("yandex-playback".into()).spawn(move || run(&id, &worker)).ok();
    *SESSION.guard() = Some(Session { control, thread, paused: false });
}

pub fn stop() {
    let Some(session) = SESSION.guard().take() else {
        return;
    };
    session.control.kill();
    let track = CURRENT.guard().take();
    if !*session.control.opened.guard() {
        return;
    }
    let host = Host::get();
    let output = host.output();
    let seconds = output.map_or(0, |output| output.output_time().max(0) / 1000);
    if let Some(thread) = session.thread {
        let _ = thread.join();
    }
    if let Some(output) = output {
        output.close();
    }
    host.visual_deinit();
    if let Some(track) = track.filter(|_| seconds >= 1) {
        library::report(track, seconds as u32);
    }
}

pub fn current() -> Option<Track> {
    CURRENT.guard().clone()
}

fn with_output(action: impl FnOnce(Output)) {
    let ready = SESSION.guard().as_ref().is_some_and(|session| session.control.ready.load(Ordering::Acquire));
    if let Some(output) = Host::get().output().filter(|_| ready) {
        action(output);
    }
}

pub fn set_paused(paused: bool) {
    if let Some(session) = SESSION.guard().as_mut() {
        session.paused = paused;
    }
    with_output(|output| output.pause(paused));
}

pub fn is_paused() -> bool {
    SESSION.guard().as_ref().is_some_and(|session| session.paused)
}

pub fn length_ms() -> i32 {
    CURRENT.guard().as_ref().map_or(-1, tracks::length_ms)
}

pub fn output_time() -> i32 {
    let mut time = 0;
    with_output(|output| time = output.output_time());
    time
}

pub fn seek(position_ms: i32) {
    if let Some(session) = SESSION.guard().as_ref() {
        session.control.seek.store(i64::from(position_ms.max(0)), Ordering::Release);
    }
}

pub fn set_volume(volume: i32) {
    if let Some(output) = Host::get().output() {
        output.set_volume(volume);
    }
}

pub fn set_pan(pan: i32) {
    if let Some(output) = Host::get().output() {
        output.set_pan(pan);
    }
}

fn run(id: &str, control: &Control) {
    let result = stream(id, control);
    if control.killed() {
        return;
    }
    match result {
        Ok(()) => Host::get().winamp().finished(),
        Err(error) => fail(id, &error),
    }
}

fn fail(id: &str, error: &Error) {
    log::write(format_args!("playing {id}: {error}"));
    let winamp = Host::get().winamp();
    let failures = FAILURES.fetch_add(1, Ordering::AcqRel) + 1;
    if error.kind() != ErrorKind::SignedOut && failures < SKIP_LIMIT {
        return winamp.finished();
    }
    FAILURES.store(0, Ordering::Release);
    winamp.command(COMMAND_STOP);
    let title = library::cached(id).map_or_else(|| id.to_owned(), |track| tracks::display(&track));
    let message = text::text_with("winamp-play-failed", &[("title", &title), ("error", &text::error(error))]);
    winamp.message(&message);
}

fn stream(id: &str, control: &Control) -> Result<()> {
    let track = library::track(id)?;
    {
        let mut current = CURRENT.guard();
        if control.killed() {
            return Ok(());
        }
        *current = Some(track.clone());
    }
    let mut position = control.take_seek().unwrap_or(0);
    let mut output = None;
    let mut retries = 0;
    let quality = library::quality();
    let mut prepared = prefetch::take(id, quality);
    loop {
        let library::Source { buffer, codec } = match prepared.take() {
            Some(source) => source,
            None => library::source(id, quality)?,
        };
        let opened = tactus_stream::open(&buffer, codec)?;
        if !control.watch(opened.cancel) {
            return Ok(());
        }
        let mut decoder = opened.decoder;
        if position > 0 {
            let _ = decoder.try_seek(Duration::from_millis(position));
        }
        let output = match output {
            Some(output) => output,
            None => match start_output(&decoder, &buffer, &track, control)? {
                Some(started) => *output.insert(started),
                None => return Ok(()),
            },
        };
        let before = position;
        match pump(&mut decoder, &buffer, output, control, &mut position) {
            Pumped::Finished | Pumped::Killed => return Ok(()),
            Pumped::Broken => {
                if position > before {
                    retries = 0;
                }
                let Some(&delay) = RETRY_DELAYS.get(retries) else {
                    return Err(Error::Audio("the stream stopped".into()));
                };
                retries += 1;
                log::write(format_args!("stream {id} broke at {position} ms, reconnecting"));
                control.rest(delay);
                if control.killed() {
                    return Ok(());
                }
            }
        }
    }
}

fn kilobits(buffer: &Buffer, track: &Track) -> i32 {
    match (buffer.total(), track.duration_ms) {
        (Some(bytes), length) if length > 0 => (bytes * 8 / length) as i32,
        _ => -1,
    }
}

fn start_output(
    decoder: &Decoder<Reader>,
    buffer: &Buffer,
    track: &Track,
    control: &Control,
) -> Result<Option<Output>> {
    let host = Host::get();
    let output = host.output().ok_or_else(|| Error::Audio("Winamp has no output plugin".into()))?;
    let rate = decoder.sample_rate().get() as i32;
    let channels = i32::from(decoder.channels().get());
    let mut opened = control.opened.guard();
    if control.killed() {
        return Ok(None);
    }
    let latency = output.open(rate, channels);
    if latency < 0 {
        return Err(Error::Audio("the Winamp output plugin could not start".into()));
    }
    host.visual_init(latency, rate);
    host.visual_info(rate, channels);
    host.info(kilobits(buffer, track), rate / 1000, channels > 1);
    output.set_volume(-666);
    *opened = true;
    control.ready.store(true, Ordering::Release);
    FAILURES.store(0, Ordering::Release);
    Ok(Some(output))
}

fn pcm(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
}

fn drain(output: Output, control: &Control) -> Option<Pumped> {
    loop {
        if control.killed() {
            return Some(Pumped::Killed);
        }
        if control.seek_pending() {
            return None;
        }
        if !output.is_playing() {
            return Some(Pumped::Finished);
        }
        std::thread::sleep(POLL);
    }
}

fn pump(
    decoder: &mut Decoder<Reader>,
    buffer: &Buffer,
    output: Output,
    control: &Control,
    position: &mut u64,
) -> Pumped {
    let host = Host::get();
    let channels = usize::from(decoder.channels().get());
    let rate = decoder.sample_rate().get();
    let mut samples = vec![0i16; BLOCK * channels * 2];
    let (mut base, mut frames_done) = (*position, 0u64);
    loop {
        if control.killed() {
            return Pumped::Killed;
        }
        if let Some(target) = control.take_seek() {
            let _ = decoder.try_seek(Duration::from_millis(target));
            output.flush(target.min(i32::MAX as u64) as i32);
            (base, frames_done, *position) = (target, 0, target);
            continue;
        }
        let mut count = 0;
        while count < BLOCK * channels {
            let Some(sample) = decoder.next() else {
                break;
            };
            samples[count] = pcm(sample);
            count += 1;
        }
        let frames = count / channels;
        if frames == 0 {
            if control.killed() {
                return Pumped::Killed;
            }
            if buffer.failed() {
                return Pumped::Broken;
            }
            match drain(output, control) {
                Some(ended) => return ended,
                None => continue,
            }
        }
        let bytes = frames * channels * 2;
        let needed = if host.dsp_active() { bytes * 2 } else { bytes };
        while output.can_write() < needed && !control.killed() && !control.seek_pending() {
            std::thread::sleep(POLL);
        }
        if control.killed() {
            return Pumped::Killed;
        }
        if control.seek_pending() {
            continue;
        }
        host.add_pcm(&mut samples, channels, output.written_time());
        let produced = host.process(&mut samples, frames, channels, rate);
        output.write(&samples[..produced * channels]);
        frames_done += frames as u64;
        *position = base + frames_done * 1000 / u64::from(rate);
    }
}
