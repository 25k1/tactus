use std::ffi::c_void;
use tactus_api::error::{Error, Result};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn check(code: u32) -> Result<()> {
    match code {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        code => Err(Error::Io(std::io::Error::from_raw_os_error(code as i32))),
    }
}

pub fn exists(key: &str, name: &str) -> bool {
    let (key, name) = (wide(key), wide(name));
    let mut size = 0u32;
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    code == ERROR_SUCCESS
}

pub fn set(key: &str, name: &str, value: &str) -> Result<()> {
    let (key, name, value) = (wide(key), wide(name), wide(value));
    check(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            REG_SZ,
            value.as_ptr().cast::<c_void>(),
            (value.len() * 2) as u32,
        )
    })
}

pub fn remove(key: &str, name: &str) -> Result<()> {
    let (key, name) = (wide(key), wide(name));
    check(unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) })
}
