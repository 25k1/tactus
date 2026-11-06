use crate::wide::wide;
use std::ffi::CStr;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

pub const INPUT_MODULE: &str = "in_yandex.dll";
pub const GENERAL_MODULE: &str = "gen_yandex.dll";
pub const REMEMBER_TRACKS: &CStr = c"tactusRememberTracks";
pub const SHOW_SETTINGS: &CStr = c"tactusShowSettings";

pub type RememberTracks = unsafe extern "C" fn(*const u8, usize);
pub type ShowSettings = unsafe extern "C" fn(isize);

pub unsafe fn function<F: Copy>(module: &str, name: &CStr) -> Option<F> {
    assert_eq!(size_of::<F>(), size_of::<usize>());
    let name_wide = wide(module);
    let module = unsafe { GetModuleHandleW(name_wide.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let function = unsafe { GetProcAddress(module, name.as_ptr().cast()) }?;
    Some(unsafe { std::mem::transmute_copy(&function) })
}
