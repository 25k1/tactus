use windows_sys::Win32::UI::Controls::EM_SETSEL;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, SendMessageW, WM_UNDO};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Undo,
    SelectAll,
}

pub fn focused(command: Edit) -> bool {
    let window = unsafe { GetFocus() };
    if window.is_null() {
        return false;
    }
    let mut name = [0u16; 16];
    let length = unsafe { GetClassNameW(window, name.as_mut_ptr(), name.len() as i32) };
    if !String::from_utf16_lossy(&name[..length.max(0) as usize]).eq_ignore_ascii_case("Edit") {
        return false;
    }
    let (message, first, last) = match command {
        Edit::Undo => (WM_UNDO, 0, 0),
        Edit::SelectAll => (EM_SETSEL, 0, -1),
    };
    unsafe { SendMessageW(window, message, first, last) };
    true
}
