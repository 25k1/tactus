use super::*;
use tactus_api::ynison::{Device, Identity, Incoming, Link, Outgoing, Playable, Playing, Shared};

const STATUS_INTERVAL: Duration = Duration::from_secs(10);
const SEEK_TOLERANCE_MS: u64 = 3_000;

pub trait Remote {
    fn send(&self, outgoing: Outgoing);
    fn take(&self) -> Vec<Incoming>;
}

impl Remote for Link {
    fn send(&self, outgoing: Outgoing) {
        Link::send(self, outgoing);
    }

    fn take(&self) -> Vec<Incoming> {
        Link::take(self)
    }
}

#[derive(Default)]
pub struct Ynison {
    pub remote: Option<Box<dyn Remote>>,
    pub shared: Option<Shared>,
    pub problem: Option<String>,
    sent_queue: Option<(Vec<Playable>, Option<usize>)>,
    sent_status: Option<Playing>,
    sent_at: Option<Instant>,
}

pub fn default_device_name() -> String {
    format!("Tactus on {}", std::env::var("COMPUTERNAME").unwrap_or_else(|_| "this PC".into()))
}

impl Player {
    pub fn devices(&self) -> &[Device] {
        self.ynison.shared.as_ref().map(|shared| shared.devices.as_slice()).unwrap_or_default()
    }

    pub fn active_device(&self) -> Option<&str> {
        self.ynison.shared.as_ref()?.active.as_deref()
    }

    pub fn device_id(&self) -> &str {
        &self.settings.device_id
    }

    pub fn ynison_problem(&self) -> Option<&str> {
        self.ynison.problem.as_deref()
    }

    pub(super) fn has_remote(&self) -> bool {
        self.ynison.remote.is_some()
    }

    fn is_active_device(&self) -> bool {
        self.active_device() == Some(self.device_id())
    }

    pub fn attach_remote(&mut self, remote: Box<dyn Remote>) {
        self.ynison = Ynison { remote: Some(remote), ..Ynison::default() };
        self.publish_full(false);
    }

    pub(super) fn connect_ynison(&mut self) {
        self.ynison = Ynison::default();
        let Some(token) = settings::load_token().filter(|_| self.settings.ynison) else {
            return;
        };
        if self.settings.device_id.is_empty() {
            let id = format!("tactus-{:016x}", seed());
            self.update_settings(|settings| settings.device_id = id);
        }
        let name = Some(self.settings.device_name.trim()).filter(|name| !name.is_empty());
        let title = name.map_or_else(default_device_name, str::to_owned);
        self.attach_remote(Box::new(Link::start(token, Identity { id: self.settings.device_id.clone(), title })));
    }

    fn local_queue(&self) -> (Vec<Playable>, Option<usize>) {
        let queue = self
            .queue
            .tracks()
            .iter()
            .map(|track| Playable { id: track.id.clone(), album_id: track.album_id.clone() })
            .collect();
        (queue, (!self.queue.is_empty()).then(|| self.queue.index()))
    }

    fn local_playing(&self) -> Playing {
        Playing {
            paused: !self.playing,
            progress_ms: self.position_ms,
            duration_ms: self.track.as_ref().map_or(0, |track| track.duration_ms),
            speed: f64::from(self.speed()),
        }
    }

    fn send(&mut self, outgoing: Outgoing) {
        if let Some(remote) = &self.ynison.remote {
            remote.send(outgoing);
        }
    }

    fn publish_full(&mut self, active: bool) {
        let (queue, index) = self.local_queue();
        let playing = self.local_playing();
        self.ynison.sent_queue = Some((queue.clone(), index));
        self.ynison.sent_status = Some(playing.clone());
        self.ynison.sent_at = Some(Instant::now());
        self.send(Outgoing::Full { queue, index, playing, active });
    }

    pub(super) fn poll_ynison(&mut self) {
        let incoming = self.ynison.remote.as_ref().map(|remote| remote.take()).unwrap_or_default();
        for event in incoming {
            match event {
                Incoming::State(shared) => self.apply_shared(shared),
                Incoming::Unavailable(problem) => {
                    log::warn(format_args!("ynison: {problem}"));
                    self.ynison.problem = Some(problem);
                    self.emit(Event::Devices);
                }
            }
        }
        if self.ynison.remote.is_none() {
            return;
        }
        if self.playing && !self.is_active_device() && self.ynison.shared.is_some() {
            let id = self.device_id().to_owned();
            self.send(Outgoing::Activate(id));
            self.publish_full(true);
            return;
        }
        if !self.is_active_device() {
            return;
        }
        let queue = self.local_queue();
        if self.ynison.sent_queue.as_ref() != Some(&queue) {
            let playing = self.local_playing();
            self.ynison.sent_queue = Some(queue.clone());
            self.ynison.sent_status = Some(playing.clone());
            self.ynison.sent_at = Some(Instant::now());
            self.send(Outgoing::Queue { queue: queue.0, index: queue.1, playing });
            return;
        }
        let playing = self.local_playing();
        let paused_changed = self.ynison.sent_status.as_ref().is_none_or(|sent| sent.paused != playing.paused);
        let stale = self.ynison.sent_at.is_none_or(|at| at.elapsed() >= STATUS_INTERVAL);
        if paused_changed || (stale && self.playing) {
            self.ynison.sent_status = Some(playing.clone());
            self.ynison.sent_at = Some(Instant::now());
            self.send(Outgoing::Status(playing));
        }
    }

    fn apply_shared(&mut self, shared: Shared) {
        let was_active = self.is_active_device();
        self.ynison.problem = None;
        self.ynison.shared = Some(shared.clone());
        let active = self.is_active_device();
        if was_active && !active && self.playing {
            self.set_playing(false);
        }
        if active {
            let local = self.local_queue();
            let sent_index = self.ynison.sent_queue.as_ref().and_then(|(_, index)| *index);
            if shared.index != local.1
                && shared.index != sent_index
                && shared.queue == local.0
                && let Some(index) = shared.index
            {
                self.ynison.sent_queue = Some((local.0, Some(index)));
                self.play_index(index);
            }
            let sent = self.ynison.sent_status.clone().unwrap_or_default();
            if shared.playing.paused != sent.paused && shared.playing.paused == self.playing {
                self.ynison.sent_status = Some(Playing { paused: shared.playing.paused, ..sent.clone() });
                self.set_playing(!shared.playing.paused);
            }
            if shared.playing.progress_ms.abs_diff(sent.progress_ms) > SEEK_TOLERANCE_MS
                && shared.playing.progress_ms.abs_diff(self.position_ms) > SEEK_TOLERANCE_MS
            {
                self.ynison.sent_status = Some(Playing { progress_ms: shared.playing.progress_ms, ..sent });
                self.seek(shared.playing.progress_ms);
            }
        }
        self.emit(Event::Devices);
    }

    pub fn play_here(&mut self) {
        let Some(shared) = self.ynison.shared.clone() else { return };
        let ids: Vec<String> = shared.queue.iter().map(|playable| playable.id.clone()).collect();
        if ids.is_empty() {
            return;
        }
        let (index, position) = (shared.index.unwrap_or_default(), shared.playing.progress_ms);
        let id = self.device_id().to_owned();
        self.send(Outgoing::Activate(id));
        self.request(
            move |service, _| service.tracks(&ids),
            move |player, tracks| {
                player.queue.replace(tracks, index);
                player.emit(Event::Queue);
                player.open_current(true, position);
                player.publish_full(true);
            },
        );
    }

    pub fn move_to(&mut self, device: &str) {
        self.publish_full(false);
        self.send(Outgoing::Activate(device.to_owned()));
        if self.playing {
            self.set_playing(false);
        }
    }

    pub fn remote_toggle(&mut self) {
        let Some(shared) = self.ynison.shared.clone() else { return };
        self.send(Outgoing::Status(Playing { paused: !shared.playing.paused, ..shared.playing }));
    }

    pub fn remote_skip(&mut self, forward: bool) {
        let Some(shared) = self.ynison.shared.clone() else { return };
        let Some(index) = shared.index else { return };
        let index = if forward { index + 1 } else { index.saturating_sub(1) };
        if index < shared.queue.len() {
            let playing = Playing { progress_ms: 0, paused: false, ..shared.playing };
            self.send(Outgoing::Queue { queue: shared.queue, index: Some(index), playing });
        }
    }
}
