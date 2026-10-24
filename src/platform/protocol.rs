use super::registry;
use tactus_api::error::Result;

const KEY: &str = r"Software\Classes\yandexmusic";
const COMMAND: &str = r"Software\Classes\yandexmusic\shell\open\command";
const PREVIOUS: &str = "TactusPrevious";

fn command() -> Result<String> {
    Ok(format!("\"{}\" \"%1\"", std::env::current_exe()?.display()))
}

fn ours(value: &str) -> bool {
    command().is_ok_and(|ours| ours.eq_ignore_ascii_case(value))
}

pub fn is_enabled() -> bool {
    registry::get(COMMAND, "").is_some_and(|value| ours(&value))
}

pub fn set(enabled: bool) -> Result<()> {
    if enabled {
        if let Some(previous) = registry::get(COMMAND, "").filter(|value| !ours(value)) {
            registry::set(KEY, PREVIOUS, &previous)?;
        }
        registry::set(KEY, "", "URL:Yandex Music")?;
        registry::set(KEY, "URL Protocol", "")?;
        return registry::set(COMMAND, "", &command()?);
    }
    if !is_enabled() {
        return Ok(());
    }
    match registry::get(KEY, PREVIOUS) {
        Some(previous) => {
            registry::set(COMMAND, "", &previous)?;
            registry::remove(KEY, PREVIOUS)
        }
        None => registry::remove_tree(KEY),
    }
}
