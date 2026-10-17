use std::fmt::Display;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const LIMIT: u64 = 1024 * 1024;

static FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

pub fn init(path: PathBuf) {
    *FILE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(path);
}

pub fn path() -> Option<PathBuf> {
    FILE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

fn line(level: Level, message: &dyn Display) -> String {
    format!("{} {level:?} {message}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
}

pub fn write(level: Level, message: impl Display) {
    let guard = FILE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(path) = guard.as_ref() else {
        return;
    };
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > LIMIT) {
        let _ = std::fs::rename(path, path.with_extension("log.old"));
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line(level, &message).as_bytes());
    }
}

pub fn info(message: impl Display) {
    write(Level::Info, message);
}

pub fn warn(message: impl Display) {
    write(Level::Warn, message);
}

pub fn error(message: impl Display) {
    write(Level::Error, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_lines_with_level() {
        let text = line(Level::Warn, &"disk is full");
        assert!(text.ends_with(" Warn disk is full\n"));
        assert_eq!(text.len(), "2026-01-01 00:00:00 Warn disk is full\n".len());
    }
}
