use super::{Gains, Loaded, Loader, Output, Start};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tactus_api::error::Result;

#[derive(Debug, Default)]
pub struct State {
    pub starts: Vec<Start>,
    pub restarts: usize,
    pub loaded: bool,
    pub playing: bool,
    pub position: Duration,
    pub finished: bool,
    pub volume: u8,
    pub speed: f32,
    pub gains: Gains,
    pub device: String,
    pub fades: Vec<Duration>,
}

#[derive(Clone, Default)]
pub struct MockOutput(pub Rc<RefCell<State>>);

pub fn loader() -> Loader {
    Arc::new(|_| Ok(Loaded::from_source(Box::new(rodio::source::Empty::new()))))
}

impl MockOutput {
    fn begin(&self, start: Start) {
        let mut state = self.0.borrow_mut();
        state.starts.push(start);
        state.loaded = true;
        state.playing = start.playing;
        state.position = start.at;
        state.finished = false;
    }
}

impl Output for MockOutput {
    fn start(&mut self, _track: Loaded, start: Start) -> Result<()> {
        self.begin(start);
        Ok(())
    }

    fn restart(&mut self, start: Start) -> Result<()> {
        self.0.borrow_mut().restarts += 1;
        self.begin(start);
        Ok(())
    }

    fn stop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.loaded = false;
        state.playing = false;
    }

    fn is_loaded(&self) -> bool {
        self.0.borrow().loaded
    }

    fn set_playing(&mut self, playing: bool) {
        self.0.borrow_mut().playing = playing;
    }

    fn position(&self) -> Duration {
        self.0.borrow().position
    }

    fn seek(&mut self, position: Duration) {
        self.0.borrow_mut().position = position;
    }

    fn finished(&self) -> bool {
        self.0.borrow().finished
    }

    fn set_volume(&mut self, volume: u8) {
        self.0.borrow_mut().volume = volume;
    }

    fn set_speed(&mut self, speed: f32) {
        self.0.borrow_mut().speed = speed;
    }

    fn set_equalizer(&mut self, gains: &Gains) {
        self.0.borrow_mut().gains = *gains;
    }

    fn set_device(&mut self, device: &str) -> Result<()> {
        device.clone_into(&mut self.0.borrow_mut().device);
        Ok(())
    }

    fn fade_out(&mut self, duration: Duration) {
        self.0.borrow_mut().fades.push(duration);
    }

    fn tick(&mut self) {}
}
