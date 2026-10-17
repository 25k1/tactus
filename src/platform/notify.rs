use super::registry;
use tactus_api::error::{Error, Result};
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::core::HSTRING;

const APP_ID: &str = "Tactus.Player";
const ICON: &[u8] = include_bytes!("../../assets/tactus.png");

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn template(title: &str, body: &str) -> String {
    format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual><audio silent="true"/></toast>"#,
        escape(title),
        escape(body)
    )
}

fn native(error: windows::core::Error) -> Error {
    Error::Api(error.message().clone())
}

pub fn enable() {
    if let Err(error) = register() {
        tactus_core::log::warn(format_args!("notifications: {error}"));
    }
}

fn register() -> Result<()> {
    let key = format!(r"Software\Classes\AppUserModelId\{APP_ID}");
    let icon = tactus_core::settings::data_dir().join("tactus.png");
    if !icon.exists() {
        std::fs::create_dir_all(tactus_core::settings::data_dir())?;
        std::fs::write(&icon, ICON)?;
    }
    registry::set(&key, "DisplayName", "Tactus")?;
    registry::set(&key, "IconUri", &icon.display().to_string())
}

pub fn show(title: &str, body: &str) -> Result<()> {
    let document = XmlDocument::new().map_err(native)?;
    document.LoadXml(&HSTRING::from(template(title, body))).map_err(native)?;
    let toast = ToastNotification::CreateToastNotification(&document).map_err(native)?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID)).map_err(native)?;
    notifier.Show(&toast).map_err(native)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_toast_text() {
        let xml = template("Rock & <Roll>", "\"Quoted\"");
        assert!(xml.contains("Rock &amp; &lt;Roll&gt;"));
        assert!(xml.contains("&quot;Quoted&quot;"));
    }
}
