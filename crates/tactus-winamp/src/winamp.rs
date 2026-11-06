use crate::sdk::*;
use crate::{tracks, wide};
use std::path::PathBuf;
use tactus_api::proxy::{Proxy, ProxyKind};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HMENU, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MESSAGEBOX_STYLE, MessageBoxW, PostMessageW,
    SendMessageW, WM_COMMAND, WM_COPYDATA,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Winamp(isize);

impl Winamp {
    pub fn new(window: HWND) -> Self {
        Self(window as isize)
    }

    pub fn window(self) -> HWND {
        self.0 as HWND
    }

    pub fn send(self, data: usize, message: u32) -> isize {
        unsafe { SendMessageW(self.window(), WM_WA_IPC, data, message as isize) }
    }

    fn ansi(self, message: u32) -> Option<String> {
        let pointer = self.send(0, message);
        if pointer <= 1 {
            return None;
        }
        let text = unsafe { wide::read_ansi(pointer as *const _) };
        (!text.trim().is_empty()).then_some(text)
    }

    pub fn state(self) -> State {
        match self.send(0, IPC_ISPLAYING) {
            1 => State::Playing,
            3 => State::Paused,
            _ => State::Stopped,
        }
    }

    pub fn position_ms(self) -> i64 {
        self.send(0, IPC_GETOUTPUTTIME) as i64
    }

    pub fn length_ms(self) -> i64 {
        self.send(2, IPC_GETOUTPUTTIME) as i64
    }

    pub fn len(self) -> usize {
        self.send(0, IPC_GETLISTLENGTH).max(0) as usize
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn index(self) -> usize {
        self.send(0, IPC_GETLISTPOS).max(0) as usize
    }

    pub fn file(self, index: usize) -> Option<String> {
        let pointer = self.send(index, IPC_GETPLAYLISTFILEW) as *const u16;
        let file = unsafe { wide::read(pointer) };
        (!file.is_empty()).then_some(file)
    }

    pub fn title(self, index: usize) -> Option<String> {
        let title = unsafe { wide::read(self.send(index, IPC_GETPLAYLISTTITLEW) as *const u16) };
        (!title.is_empty()).then_some(title)
    }

    pub fn current_file(self) -> Option<String> {
        if self.is_empty() { None } else { self.file(self.index()) }
    }

    pub fn current_track_id(self) -> Option<String> {
        self.current_file().as_deref().and_then(tracks::id)
    }

    pub fn following(self) -> Option<usize> {
        if self.send(0, IPC_GET_SHUFFLE) != 0 {
            return None;
        }
        let (next, len) = (self.index() + 1, self.len());
        if next < len { Some(next) } else { (len > 0 && self.send(0, IPC_GET_REPEAT) != 0).then_some(0) }
    }

    pub fn following_track_id(self) -> Option<String> {
        self.following().and_then(|index| self.file(index)).as_deref().and_then(tracks::id)
    }

    pub fn clear(self) {
        self.send(0, IPC_DELETE);
    }

    pub fn select(self, index: usize) {
        self.send(index, IPC_SETPLAYLISTPOS);
    }

    pub fn enqueue(self, location: &str) {
        let text = wide::wide(location);
        copy_data(self.window(), IPC_PLAYFILEW, text.as_ptr().cast(), text.len() * 2);
    }

    pub fn insert(self, index: usize, location: &str) {
        let editor = self.send(IPC_GETWND_PE, IPC_GETWND) as HWND;
        if editor.is_null() {
            return self.enqueue(location);
        }
        let mut info = FileInfoW { file: [0; MAX_PATH], index: index as i32 };
        unsafe { wide::write(info.file.as_mut_ptr(), MAX_PATH, location) };
        copy_data(editor, IPC_PE_INSERTFILENAMEW, (&raw const info).cast(), size_of::<FileInfoW>());
    }

    pub fn command(self, command: usize) {
        unsafe { PostMessageW(self.window(), WM_COMMAND, command, 0) };
    }

    pub fn finished(self) {
        unsafe { PostMessageW(self.window(), WM_WA_MPEG_EOF, 0, 0) };
    }

    pub fn ini_directory(self) -> Option<PathBuf> {
        self.ansi(IPC_GETINIDIRECTORY).map(PathBuf::from)
    }

    pub fn proxy(self) -> Option<String> {
        self.ansi(IPC_GET_PROXY_STRING)
    }

    pub fn register_command(self) -> u16 {
        self.send(0, IPC_REGISTER_LOWORD_COMMAND) as u16
    }

    pub fn register_message(self, name: &std::ffi::CStr) -> u32 {
        self.send(name.as_ptr() as usize, IPC_REGISTER_WINAMP_IPCMESSAGE) as u32
    }

    pub fn main_menu(self) -> HMENU {
        self.send(0, IPC_GET_HMENU) as HMENU
    }

    pub fn message(self, text: &str) {
        message(self.window(), text);
    }
}

fn copy_data(target: HWND, kind: usize, data: *const u8, length: usize) {
    let data = COPYDATASTRUCT { dwData: kind, cbData: length as u32, lpData: data.cast_mut().cast() };
    unsafe { SendMessageW(target, WM_COPYDATA, 0, &raw const data as isize) };
}

fn dialog(owner: HWND, text: &str, style: MESSAGEBOX_STYLE) -> i32 {
    let (text, title) = (wide::wide(text), wide::wide(&crate::text::text("winamp-title")));
    unsafe { MessageBoxW(owner, text.as_ptr(), title.as_ptr(), style) }
}

pub fn message(owner: HWND, text: &str) {
    dialog(owner, text, MB_OK | MB_ICONINFORMATION);
}

pub fn ask(owner: HWND, text: &str) -> bool {
    dialog(owner, text, MB_YESNO | MB_ICONQUESTION) == IDYES
}

pub fn parse_proxy(text: &str) -> Option<Proxy> {
    let text = text.trim();
    let text = text.split_once("://").map_or(text, |(_, rest)| rest).trim_end_matches('/');
    let (credentials, address) = text.rsplit_once('@').map_or(("", text), |(login, address)| (login, address));
    let (host, port) = address.rsplit_once(':')?;
    let (login, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let port = port.parse().ok()?;
    (!host.is_empty()).then(|| Proxy {
        enabled: true,
        kind: ProxyKind::Http,
        host: host.to_owned(),
        port,
        login: login.to_owned(),
        password: password.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_winamp_proxy_strings() {
        let proxy = parse_proxy("proxy.local:3128").unwrap();
        assert_eq!((proxy.host.as_str(), proxy.port, proxy.enabled), ("proxy.local", 3128, true));
        let proxy = parse_proxy("http://user:secret@10.0.0.1:8080/").unwrap();
        assert_eq!(
            (proxy.login.as_str(), proxy.password.as_str(), proxy.host.as_str()),
            ("user", "secret", "10.0.0.1")
        );
        assert_eq!(parse_proxy("proxy.local"), None);
        assert_eq!(parse_proxy(":80"), None);
    }
}
