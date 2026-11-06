use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tactus_api::sync::Guard;

const LIMIT: u64 = 512 * 1024;

static PATH: OnceLock<PathBuf> = OnceLock::new();
static WRITING: Mutex<()> = Mutex::new(());

pub fn init(dir: &Path) {
    let _ = PATH.set(dir.join("yandex.log"));
}

pub fn write(message: impl std::fmt::Display) {
    let Some(path) = PATH.get() else {
        return;
    };
    let _writing = WRITING.guard();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > LIMIT) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {message}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}
