use super::controls;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const OK: u16 = IDOK as u16;
pub const CANCEL: u16 = IDCANCEL as u16;

const WM_WAKE: u32 = WM_APP + 1;
const DWLP_USER: i32 = (2 * size_of::<isize>()) as i32;
const MARGIN: i32 = 10;
const LABEL_HEIGHT: i32 = 18;
const ROW: i32 = 24;
const BUTTON_WIDTH: i32 = 120;

pub trait Page {
    fn build(&mut self, layout: &mut Layout) -> HWND;

    fn command(&mut self, _dialog: HWND, _id: u16) {}

    fn accept(&mut self, _dialog: HWND) -> bool {
        true
    }

    fn update(&mut self, _dialog: HWND) {}
}

pub struct Layout {
    dialog: HWND,
    width: i32,
    y: i32,
}

impl Layout {
    pub fn dialog(&self) -> HWND {
        self.dialog
    }

    pub fn field(&mut self, label: &str, height: i32, create: impl FnOnce(HWND) -> HWND) -> HWND {
        controls::place(controls::label(self.dialog, label), MARGIN, self.y, self.width, LABEL_HEIGHT);
        let control = create(self.dialog);
        controls::place(control, MARGIN, self.y + LABEL_HEIGHT, self.width, height);
        self.y += LABEL_HEIGHT + height + MARGIN;
        control
    }

    pub fn line(&mut self, label: &str, create: impl FnOnce(HWND) -> HWND) -> HWND {
        self.field(label, ROW, create)
    }

    pub fn buttons(&mut self, buttons: &[(u16, &str)]) -> Vec<HWND> {
        let count = buttons.len() as i32;
        let mut x = MARGIN + self.width - count * BUTTON_WIDTH - (count - 1) * MARGIN;
        let created = buttons
            .iter()
            .map(|(id, label)| {
                let button = controls::button(self.dialog, *id, label);
                controls::place(button, x, self.y, BUTTON_WIDTH, ROW);
                x += BUTTON_WIDTH + MARGIN;
                button
            })
            .collect();
        self.y += ROW + MARGIN;
        created
    }

    fn fit(&self, owner: HWND) {
        let size = |value| value * controls::dpi(self.dialog) / 96;
        let mut frame = RECT { left: 0, top: 0, right: size(self.width + MARGIN * 2), bottom: size(self.y) };
        let style = unsafe { GetWindowLongW(self.dialog, GWL_STYLE) } as u32;
        unsafe { AdjustWindowRectEx(&raw mut frame, style, 0, 0) };
        let (width, height) = (frame.right - frame.left, frame.bottom - frame.top);
        let anchor = if owner.is_null() { self.dialog } else { owner };
        let mut area = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(anchor, &raw mut area) };
        let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..unsafe { std::mem::zeroed() } };
        unsafe { GetMonitorInfoW(MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST), &raw mut monitor) };
        let work = monitor.rcWork;
        let centered = |start: i32, end: i32, size: i32, low: i32, high: i32| {
            (start + (end - start - size) / 2).min(high - size).max(low)
        };
        let x = centered(area.left, area.right, width, work.left, work.right);
        let y = centered(area.top, area.bottom, height, work.top, work.bottom);
        unsafe { SetWindowPos(self.dialog, null_mut(), x, y, width, height, SWP_NOZORDER) };
    }
}

struct Session<'a> {
    page: &'a mut dyn Page,
    owner: HWND,
    width: i32,
}

pub fn show(owner: HWND, title: &str, width: i32, page: &mut dyn Page) -> bool {
    let mut session = Session { page, owner, width };
    let template = template(title);
    let result = unsafe {
        DialogBoxIndirectParamW(
            GetModuleHandleW(null_mut()),
            template.as_ptr().cast(),
            owner,
            Some(procedure),
            &raw mut session as LPARAM,
        )
    };
    result == isize::from(OK as i16)
}

pub fn wake(dialog: isize) {
    unsafe { PostMessageW(dialog as HWND, WM_WAKE, 0, 0) };
}

pub fn close(dialog: HWND, id: u16) {
    unsafe { EndDialog(dialog, isize::from(id as i16)) };
}

fn template(title: &str) -> Vec<u32> {
    let style = DS_MODALFRAME as u32 | WS_POPUP | WS_CAPTION | WS_SYSMENU;
    let mut units: Vec<u16> = Vec::new();
    units.extend([style as u16, (style >> 16) as u16, 0, 0]);
    units.extend([0, 0, 0, 100, 60]);
    units.extend([0, 0]);
    units.extend(title.encode_utf16().chain(std::iter::once(0)));
    let mut aligned = vec![0u32; units.len().div_ceil(2)];
    for (index, unit) in units.iter().enumerate() {
        let word = &mut aligned[index / 2];
        *word |= u32::from(*unit) << (16 * (index % 2));
    }
    aligned
}

unsafe fn session<'a>(dialog: HWND) -> Option<&'a mut Session<'a>> {
    let pointer = unsafe { GetWindowLongPtrW(dialog, DWLP_USER) } as *mut Session<'a>;
    unsafe { pointer.as_mut() }
}

unsafe extern "system" fn procedure(dialog: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match message {
        WM_INITDIALOG => {
            unsafe { SetWindowLongPtrW(dialog, DWLP_USER, lparam as _) };
            let Some(session) = (unsafe { session(dialog) }) else {
                return 1;
            };
            let mut layout = Layout { dialog, width: session.width, y: MARGIN };
            let focus = session.page.build(&mut layout);
            layout.fit(session.owner);
            controls::focus(focus);
            0
        }
        WM_WAKE => {
            if let Some(session) = unsafe { session(dialog) } {
                session.page.update(dialog);
            }
            1
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as u16;
            let Some(session) = (unsafe { session(dialog) }) else {
                return 0;
            };
            match id {
                OK if session.page.accept(dialog) => close(dialog, OK),
                OK => {}
                CANCEL => close(dialog, CANCEL),
                id => session.page.command(dialog, id),
            }
            1
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_an_aligned_template_without_controls() {
        let words = template("Ab");
        let units: Vec<u16> = words.iter().flat_map(|word| [*word as u16, (*word >> 16) as u16]).collect();
        let style = u32::from(units[0]) | (u32::from(units[1]) << 16);
        assert_eq!(style & WS_CAPTION, WS_CAPTION);
        assert_eq!(units[4], 0);
        assert_eq!(&units[9..11], &[0, 0]);
        assert_eq!(&units[11..14], &[u16::from(b'A'), u16::from(b'b'), 0]);
    }
}
