use tactus_api::Track;
use tactus_winamp::text::text;
use tactus_winamp::ui::controls;
use tactus_winamp::ui::dialog::{self, CANCEL, Layout, Page};
use tactus_winamp::{speech, tracks};
use windows_sys::Win32::Foundation::HWND;

const WIDTH: i32 = 460;
const DETAILS_HEIGHT: i32 = 120;
const DETAILS: u16 = 100;
const COPY_LINK: u16 = 101;
const OPEN_LINK: u16 = 102;

struct Info {
    details: String,
    link: String,
}

fn details(track: &Track) -> String {
    let lines = [
        (text("column-title"), track.full_title()),
        (text("column-artist"), track.artist.clone()),
        (text("kind-album"), track.album.clone()),
        (text("column-duration"), tracks::duration(track.duration_ms)),
    ];
    lines
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("\r\n")
}

impl Page for Info {
    fn build(&mut self, layout: &mut Layout) -> HWND {
        let details =
            layout.field(&text("action-track-info"), DETAILS_HEIGHT, |parent| controls::read_only(parent, DETAILS));
        controls::set_text(details, &self.details);
        let buttons = [
            (COPY_LINK, text("menu-copy-link")),
            (OPEN_LINK, text("winamp-open-in-browser")),
            (CANCEL, text("dialog-close")),
        ];
        let buttons: Vec<(u16, &str)> = buttons.iter().map(|(id, label)| (*id, label.as_str())).collect();
        let created = layout.buttons(&buttons);
        if self.link.is_empty() {
            created[..2].iter().for_each(|button| controls::enable(*button, false));
        }
        details
    }

    fn command(&mut self, dialog: HWND, id: u16) {
        match id {
            COPY_LINK => {
                controls::copy(dialog, &self.link);
                speech::say(text("notice-link-copied"));
            }
            OPEN_LINK => controls::open_url(&self.link),
            _ => {}
        }
    }
}

pub fn show(owner: HWND, track: &Track) {
    let mut page = Info { details: details(track), link: tracks::web_link(track) };
    dialog::show(owner, &tracks::display(track), WIDTH, &mut page);
}
