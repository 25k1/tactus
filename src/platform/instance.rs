use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use wxdragon::prelude::SingleInstanceChecker;

const RAISE: &str = "tactus:raise";
const OPEN: &str = "tactus:open:";
const MESSAGE_LIMIT: u64 = 4096;
const RESTART_WAIT: Duration = Duration::from_secs(20);
const AFTER_UPDATE: &str = "--after-update";
const RESTART: &str = "--restart";
const PLAYING: &str = "--playing";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Raise,
    Open(String),
}

static REQUESTS: Mutex<Vec<Request>> = Mutex::new(Vec::new());

fn port_file() -> PathBuf {
    tactus_core::settings::data_dir().join("instance.port")
}

fn notify_running_instance() -> bool {
    let Some(port) = std::fs::read_to_string(port_file()).ok().and_then(|text| text.trim().parse::<u16>().ok()) else {
        return false;
    };
    let message = launch_link().map_or_else(|| RAISE.to_owned(), |link| format!("{OPEN}{link}"));
    TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(2))
        .and_then(|mut stream| stream.write_all(message.as_bytes()))
        .is_ok()
}

fn request(message: &str) -> Option<Request> {
    match message.strip_prefix(OPEN) {
        Some(link) => Some(Request::Open(link.to_owned())),
        None => (message == RAISE).then_some(Request::Raise),
    }
}

pub fn launch_link() -> Option<String> {
    std::env::args().skip(1).find(|argument| !argument.starts_with("--"))
}

fn flag(name: &str) -> bool {
    std::env::args().skip(1).any(|argument| argument == name)
}

pub fn after_update() -> bool {
    flag(AFTER_UPDATE)
}

pub fn restarted() -> Option<bool> {
    (after_update() || flag(RESTART)).then(|| flag(PLAYING))
}

pub fn relaunch(path: &Path, updated: bool, playing: bool) -> std::io::Result<()> {
    let mut command = std::process::Command::new(path);
    command.arg(if updated { AFTER_UPDATE } else { RESTART });
    if playing {
        command.arg(PLAYING);
    }
    command.spawn().map(drop)
}

pub fn acquire() -> Option<SingleInstanceChecker> {
    let flavor = if cfg!(feature = "fake") { "tactus-demo" } else { "tactus" };
    let name = format!("{flavor}-{}", std::env::var("USERNAME").unwrap_or_default());
    let deadline = Instant::now() + if restarted().is_some() { RESTART_WAIT } else { Duration::ZERO };
    loop {
        let checker = SingleInstanceChecker::new(&name, None);
        if !checker.as_ref().is_some_and(SingleInstanceChecker::is_another_running) {
            return checker;
        }
        if Instant::now() >= deadline {
            notify_running_instance();
            std::process::exit(0);
        }
        drop(checker);
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn listen() {
    let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) else {
        return;
    };
    if let Ok(address) = listener.local_addr() {
        let _ = std::fs::create_dir_all(tactus_core::settings::data_dir());
        let _ = std::fs::write(port_file(), address.port().to_string());
    }
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut message = String::new();
            if stream.take(MESSAGE_LIMIT).read_to_string(&mut message).is_ok()
                && let Some(request) = request(&message)
                && let Ok(mut requests) = REQUESTS.lock()
            {
                requests.push(request);
            }
        }
    });
}

pub fn take_requests() -> Vec<Request> {
    REQUESTS.lock().map(|mut requests| std::mem::take(&mut *requests)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_raise_and_open_messages() {
        assert_eq!(request(RAISE), Some(Request::Raise));
        assert_eq!(request("tactus:open:yandexmusic://album/1"), Some(Request::Open("yandexmusic://album/1".into())));
        assert_eq!(request("GET / HTTP/1.1"), None);
    }
}
