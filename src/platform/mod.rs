pub mod autostart;
pub mod edit;
pub mod hotkeys;
pub mod instance;
pub mod notify;
mod registry;

use std::path::Path;

pub fn open(path: &Path) {
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

pub fn show_in_folder(path: &Path) {
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
}
