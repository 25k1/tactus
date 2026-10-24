use super::registry;
use tactus_api::error::Result;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Tactus";

pub fn is_enabled() -> bool {
    registry::get(RUN_KEY, VALUE).is_some()
}

pub fn set(enabled: bool) -> Result<()> {
    if !enabled {
        return registry::remove(RUN_KEY, VALUE);
    }
    let command = format!("\"{}\"", std::env::current_exe()?.display());
    registry::set(RUN_KEY, VALUE, &command)
}
