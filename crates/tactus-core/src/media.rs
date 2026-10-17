use crate::player::SEEK_STEP_MS;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection,
};
use std::ffi::c_void;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use tactus_api::Track;

pub enum MediaCommand {
    Track(Track),
    Playback { playing: bool, position_ms: u64 },
    Enabled(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekBy(i64),
    SetPosition(u64),
    Raise,
    Quit,
}

pub struct Media {
    commands: Sender<MediaCommand>,
    events: Receiver<MediaEvent>,
}

impl Media {
    pub fn start(window: usize, enabled: bool) -> Media {
        let (commands, receiver) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || worker(window, enabled, receiver, sender));
        Media { commands, events }
    }

    pub fn send(&self, command: MediaCommand) {
        let _ = self.commands.send(command);
    }

    pub fn events(&self) -> impl Iterator<Item = MediaEvent> + '_ {
        self.events.try_iter()
    }
}

fn event(event: MediaControlEvent) -> Option<MediaEvent> {
    let signed = |direction: SeekDirection, amount: i64| match direction {
        SeekDirection::Forward => amount,
        SeekDirection::Backward => -amount,
    };
    Some(match event {
        MediaControlEvent::Play => MediaEvent::Play,
        MediaControlEvent::Pause => MediaEvent::Pause,
        MediaControlEvent::Toggle => MediaEvent::Toggle,
        MediaControlEvent::Next => MediaEvent::Next,
        MediaControlEvent::Previous => MediaEvent::Previous,
        MediaControlEvent::Stop => MediaEvent::Stop,
        MediaControlEvent::Seek(direction) => MediaEvent::SeekBy(signed(direction, SEEK_STEP_MS)),
        MediaControlEvent::SeekBy(direction, amount) => {
            MediaEvent::SeekBy(signed(direction, amount.as_millis() as i64))
        }
        MediaControlEvent::SetPosition(MediaPosition(position)) => MediaEvent::SetPosition(position.as_millis() as u64),
        MediaControlEvent::Raise => MediaEvent::Raise,
        MediaControlEvent::Quit => MediaEvent::Quit,
        MediaControlEvent::SetVolume(_) | MediaControlEvent::OpenUri(_) => return None,
    })
}

fn attach(window: usize, events: &Sender<MediaEvent>) -> Option<MediaControls> {
    let config = PlatformConfig { display_name: "Tactus", dbus_name: "tactus", hwnd: Some(window as *mut c_void) };
    let mut controls = MediaControls::new(config).ok()?;
    let events = events.clone();
    controls
        .attach(move |control| {
            if let Some(event) = event(control) {
                let _ = events.send(event);
            }
        })
        .ok()?;
    Some(controls)
}

fn worker(window: usize, enabled: bool, commands: Receiver<MediaCommand>, events: Sender<MediaEvent>) {
    let mut controls = enabled.then(|| attach(window, &events)).flatten();
    let mut track: Option<Track> = None;
    for command in commands {
        match command {
            MediaCommand::Enabled(on) => {
                controls = on.then(|| attach(window, &events)).flatten();
                if let (Some(controls), Some(track)) = (controls.as_mut(), track.as_ref()) {
                    set_metadata(controls, track);
                }
            }
            MediaCommand::Track(new) => {
                if let Some(controls) = controls.as_mut() {
                    set_metadata(controls, &new);
                }
                track = Some(new);
            }
            MediaCommand::Playback { playing, position_ms } => {
                let progress = Some(MediaPosition(Duration::from_millis(position_ms)));
                let playback =
                    if playing { MediaPlayback::Playing { progress } } else { MediaPlayback::Paused { progress } };
                if let Some(controls) = controls.as_mut() {
                    let _ = controls.set_playback(playback);
                }
            }
        }
    }
}

fn text(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn set_metadata(controls: &mut MediaControls, track: &Track) {
    let title = track.full_title();
    let _ = controls.set_metadata(MediaMetadata {
        title: text(&title),
        artist: text(&track.artist),
        album: text(&track.album),
        cover_url: text(&track.cover),
        duration: (track.duration_ms > 0).then(|| Duration::from_millis(track.duration_ms)),
    });
}
