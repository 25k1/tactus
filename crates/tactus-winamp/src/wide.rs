use std::ffi::c_char;
use windows_sys::Win32::Globalization::{CP_ACP, MultiByteToWideChar, WideCharToMultiByte};

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub unsafe fn read(pointer: *const u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0;
    while unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(pointer, length) })
}

pub unsafe fn write(target: *mut u16, capacity: usize, text: &str) {
    if target.is_null() || capacity == 0 {
        return;
    }
    let units: Vec<u16> = text.encode_utf16().take(capacity - 1).collect();
    unsafe {
        std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
        *target.add(units.len()) = 0;
    }
}

pub unsafe fn read_ansi(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let bytes = unsafe { std::ffi::CStr::from_ptr(pointer) }.to_bytes();
    if bytes.is_empty() {
        return String::new();
    }
    let length = bytes.len() as i32;
    let needed = unsafe { MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), length, std::ptr::null_mut(), 0) };
    let mut units = vec![0u16; needed.max(0) as usize];
    let written = unsafe { MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), length, units.as_mut_ptr(), needed) };
    String::from_utf16_lossy(&units[..written.max(0) as usize])
}

pub fn ansi(text: &str) -> Vec<u8> {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() {
        return vec![0];
    }
    let length = units.len() as i32;
    let null = std::ptr::null();
    let needed = unsafe {
        WideCharToMultiByte(CP_ACP, 0, units.as_ptr(), length, std::ptr::null_mut(), 0, null, std::ptr::null_mut())
    };
    let mut bytes = vec![0u8; needed.max(0) as usize + 1];
    unsafe {
        WideCharToMultiByte(CP_ACP, 0, units.as_ptr(), length, bytes.as_mut_ptr(), needed, null, std::ptr::null_mut())
    };
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_wide_buffers() {
        let text = "Кино – Группа крови";
        let encoded = wide(text);
        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(unsafe { read(encoded.as_ptr()) }, text);
        let mut buffer = [0u16; 5];
        unsafe { write(buffer.as_mut_ptr(), buffer.len(), text) };
        assert_eq!(unsafe { read(buffer.as_ptr()) }, "Кино");
    }
}
