use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use wxdragon::prelude::SingleInstanceChecker;

const RAISE: &[u8] = b"tactus:raise";
const UPDATE_WAIT: Duration = Duration::from_secs(20);
pub const AFTER_UPDATE: &str = "--after-update";

static RAISE_REQUESTED: AtomicBool = AtomicBool::new(false);

fn port_file() -> PathBuf {
    tactus_core::settings::data_dir().join("instance.port")
}

fn notify_running_instance() -> bool {
    let Some(port) = std::fs::read_to_string(port_file()).ok().and_then(|text| text.trim().parse::<u16>().ok()) else {
        return false;
    };
    TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(2))
        .and_then(|mut stream| stream.write_all(RAISE))
        .is_ok()
}

pub fn after_update() -> bool {
    std::env::args().skip(1).any(|argument| argument == AFTER_UPDATE)
}

pub fn acquire() -> Option<SingleInstanceChecker> {
    let flavor = if cfg!(feature = "fake") { "tactus-demo" } else { "tactus" };
    let name = format!("{flavor}-{}", std::env::var("USERNAME").unwrap_or_default());
    let deadline = Instant::now() + if after_update() { UPDATE_WAIT } else { Duration::ZERO };
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
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut message = [0; RAISE.len()];
            if stream.read_exact(&mut message).is_ok() && message == RAISE {
                RAISE_REQUESTED.store(true, Ordering::Relaxed);
            }
        }
    });
}

pub fn take_raise_request() -> bool {
    RAISE_REQUESTED.swap(false, Ordering::Relaxed)
}
