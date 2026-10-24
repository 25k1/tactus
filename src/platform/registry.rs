use std::ffi::c_void;
use tactus_api::error::{Error, Result};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW, RegSetKeyValueW,
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

pub fn get(key: &str, name: &str) -> Option<String> {
    let (key, name) = (wide(key), wide(name));
    let query = |data: *mut c_void, size: &mut u32| unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), data, size)
    };
    let mut size = 0u32;
    if query(std::ptr::null_mut(), &mut size) != ERROR_SUCCESS {
        return None;
    }
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    if query(buffer.as_mut_ptr().cast(), &mut size) != ERROR_SUCCESS {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer).trim_end_matches('\0').to_owned())
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

pub fn remove_tree(key: &str) -> Result<()> {
    let key = wide(key);
    check(unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) })
}
