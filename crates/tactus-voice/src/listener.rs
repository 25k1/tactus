use crate::audio::{Activity, RATE};
use crate::engine::{Model, Recognizer, Vosk};
use crate::microphone::{Microphone, Source};
use crate::names::split_wake;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub runtime: PathBuf,
    pub model: PathBuf,
    pub microphone: String,
    pub wake_word: Option<String>,
    pub silence: Duration,
    pub wait: Duration,
    pub limit: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Hold,
    Tap,
    Release,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heard {
    Ready,
    Listening,
    Woke,
    Text(String),
    Nothing,
    Cancelled,
    Failed(String),
}

pub struct Listener {
    control: Sender<Control>,
    heard: Receiver<Heard>,
}

pub type Opener = Box<dyn Fn(&str) -> Result<Box<dyn Source>, String> + Send>;

impl Listener {
    pub fn start(config: Config) -> Listener {
        Self::start_with(config, Box::new(|name| Ok(Box::new(Microphone::open(name)?) as Box<dyn Source>)))
    }

    pub fn start_with(config: Config, open: Opener) -> Listener {
        let (control, inbox) = mpsc::channel();
        let (outbox, heard) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(error) = run(&config, &open, &inbox, &outbox) {
                let _ = outbox.send(Heard::Failed(error));
            }
        });
        Listener { control, heard }
    }

    pub fn send(&self, control: Control) {
        let _ = self.control.send(control);
    }

    pub fn take(&self) -> Vec<Heard> {
        self.heard.try_iter().collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hold,
    Command,
}

struct Session<'a> {
    config: &'a Config,
    inbox: &'a Receiver<Control>,
    outbox: &'a Sender<Heard>,
    command: Recognizer,
    open: &'a Opener,
    microphone: Option<Box<dyn Source>>,
}

impl Session<'_> {
    fn microphone(&mut self) -> Result<&mut Box<dyn Source>, String> {
        if self.microphone.is_none() {
            self.microphone = Some((self.open)(&self.config.microphone)?);
        }
        self.microphone.as_mut().ok_or_else(|| "no microphone".to_owned())
    }

    fn listen(&mut self, mut mode: Mode) -> Result<(), String> {
        self.microphone()?;
        let _ = self.outbox.send(Heard::Listening);
        self.command.reset();
        let (started, mut activity, mut spoke, mut last_voice) =
            (Instant::now(), Activity::default(), false, Instant::now());
        loop {
            match self.inbox.try_recv() {
                Ok(Control::Release) if mode == Mode::Hold => break,
                Ok(Control::Tap) => mode = Mode::Command,
                Ok(Control::Cancel) => {
                    self.command.reset();
                    let _ = self.outbox.send(Heard::Cancelled);
                    return Ok(());
                }
                Err(TryRecvError::Disconnected) => return Ok(()),
                _ => {}
            }
            if let Some(samples) = self.microphone()?.read(POLL) {
                if activity.speech(&samples) {
                    spoke = true;
                    last_voice = Instant::now();
                }
                let endpoint = self.command.accept(&samples);
                if mode == Mode::Command && spoke && endpoint {
                    break;
                }
            }
            let quiet = last_voice.elapsed() >= self.config.silence;
            if started.elapsed() >= self.config.limit || (mode == Mode::Command && spoke && quiet) {
                break;
            }
            if mode == Mode::Command && !spoke && started.elapsed() >= self.config.wait {
                self.command.reset();
                let _ = self.outbox.send(Heard::Nothing);
                return Ok(());
            }
        }
        let text = self.command.finish();
        let _ = self.outbox.send(if text.trim().is_empty() { Heard::Nothing } else { Heard::Text(text) });
        Ok(())
    }
}

fn run(config: &Config, open: &Opener, inbox: &Receiver<Control>, outbox: &Sender<Heard>) -> Result<(), String> {
    let vosk = Vosk::load(&config.runtime)?;
    let model = Model::load(&vosk, &config.model)?;
    let command = Recognizer::new(&model, RATE, None)?;
    let mut wake = match &config.wake_word {
        Some(word) => Some((Recognizer::new(&model, RATE, None)?, word.as_str())),
        None => None,
    };
    let mut session = Session { config, inbox, outbox, command, open, microphone: None };
    let _ = outbox.send(Heard::Ready);
    loop {
        let control = match &mut wake {
            None => {
                session.microphone = None;
                match inbox.recv() {
                    Ok(control) => Some(control),
                    Err(_) => return Ok(()),
                }
            }
            Some((recognizer, word)) => {
                let heard = match session.microphone()?.read(POLL) {
                    Some(samples) if recognizer.accept(&samples) => recognizer.result(),
                    _ => String::new(),
                };
                if let Some(rest) = split_wake(&heard, word) {
                    recognizer.reset();
                    let _ = outbox.send(Heard::Woke);
                    match rest.as_slice() {
                        [] => session.listen(Mode::Command)?,
                        words => {
                            let _ = outbox.send(Heard::Text(words.join(" ")));
                        }
                    }
                    continue;
                }
                match inbox.recv_timeout(Duration::ZERO) {
                    Ok(control) => Some(control),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            }
        };
        match control {
            Some(Control::Hold) => session.listen(Mode::Hold)?,
            Some(Control::Tap) => session.listen(Mode::Command)?,
            Some(Control::Release | Control::Cancel) | None => {}
        }
    }
}
