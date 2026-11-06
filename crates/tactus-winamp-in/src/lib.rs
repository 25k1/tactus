#![allow(non_snake_case, clippy::missing_safety_doc)]

mod host;
mod info;
mod library;
mod playback;
mod prefetch;

use std::ffi::{CStr, c_char, c_int};
use std::ptr::null_mut;
use std::time::Duration;
use tactus_api::Track;
use tactus_winamp::bridge::{self, GENERAL_MODULE, SHOW_SETTINGS, ShowSettings};
use tactus_winamp::sdk::*;
use tactus_winamp::text::{text, text_with};
use tactus_winamp::{DESCRIPTION, VERSION, tracks, wide, winamp};
use windows_sys::Win32::Foundation::HWND;

const EXTENSIONS: &[u8] = b"\0\0";
const INFO_WAIT: Duration = Duration::from_secs(3);

pub static MODULE: Exported<InModule> = Exported::new(InModule {
    version: IN_VER,
    description: DESCRIPTION.as_ptr().cast_mut().cast(),
    main_window: null_mut(),
    instance: null_mut(),
    extensions: EXTENSIONS.as_ptr().cast_mut().cast(),
    seekable: 1,
    flags: IN_MODULE_FLAG_USES_OUTPUT_PLUGIN,
    config: Some(config),
    about: Some(about),
    init: Some(init),
    quit: Some(quit),
    file_info: Some(file_info),
    info_box: Some(info_box),
    is_our_file: Some(is_our_file),
    play: Some(play),
    pause: Some(pause),
    unpause: Some(unpause),
    is_paused: Some(is_paused),
    stop: Some(stop),
    length: Some(length),
    output_time: Some(output_time),
    set_output_time: Some(set_output_time),
    set_volume: Some(set_volume),
    set_pan: Some(set_pan),
    sa_vsa_init: None,
    sa_vsa_deinit: None,
    sa_add_pcm: None,
    sa_get_mode: None,
    sa_add: None,
    vsa_add_pcm: None,
    vsa_get_mode: None,
    vsa_add: None,
    vsa_set_info: None,
    dsp_is_active: None,
    dsp_do_samples: None,
    eq_set: Some(eq_set),
    set_info: None,
    out: null_mut(),
    service: null_mut(),
    reserved: [null_mut(); 32],
});

#[unsafe(no_mangle)]
pub extern "C" fn winampGetInModule2() -> *mut InModule {
    MODULE.get()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn winampGetExtendedFileInfoW(
    file: *const u16,
    field: *const c_char,
    target: *mut u16,
    capacity: c_int,
) -> c_int {
    let Some(track) = track_id(file).and_then(|id| library::lookup(&id)) else {
        return 0;
    };
    let field = if field.is_null() { "" } else { unsafe { CStr::from_ptr(field) }.to_str().unwrap_or_default() };
    let Some(value) = field_value(&track, field) else {
        return 0;
    };
    unsafe { wide::write(target, capacity.max(0) as usize, &value) };
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tactusRememberTracks(data: *const u8, length: usize) {
    if data.is_null() {
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, length) };
    if let Ok(tracks) = serde_json::from_slice::<Vec<Track>>(bytes) {
        library::remember(tracks);
    }
}

fn field_value(track: &Track, field: &str) -> Option<String> {
    let value = match field.to_ascii_lowercase().as_str() {
        "artist" | "albumartist" => track.artist.clone(),
        "title" => track.full_title(),
        "album" => track.album.clone(),
        "genre" => track.genre.clone(),
        "year" if track.year > 0 => track.year.to_string(),
        "length" if track.duration_ms > 0 => track.duration_ms.to_string(),
        "type" => "0".to_owned(),
        "family" => text("winamp-title"),
        _ => return None,
    };
    (!value.is_empty()).then_some(value)
}

fn track_id(file: *const u16) -> Option<String> {
    tracks::id(&unsafe { wide::read(file) })
}

fn requested(file: *const u16, wait: Duration) -> Option<Track> {
    let location = unsafe { wide::read(file) };
    if location.is_empty() {
        return playback::current();
    }
    let id = tracks::id(&location)?;
    if wait.is_zero() { library::lookup(&id) } else { library::track_within(&id, wait) }
}

unsafe extern "C" fn init() {
    let store = tactus_winamp::prepare(host::Host::get().winamp());
    library::init(store);
}

unsafe extern "C" fn quit() {
    playback::stop();
}

unsafe extern "C" fn config(parent: HWND) {
    match unsafe { bridge::function::<ShowSettings>(GENERAL_MODULE, SHOW_SETTINGS) } {
        Some(show) => unsafe { show(parent as isize) },
        None => winamp::message(parent, &text("winamp-general-missing")),
    }
}

unsafe extern "C" fn about(parent: HWND) {
    winamp::message(parent, &text_with("winamp-about", &[("version", VERSION)]));
}

unsafe extern "C" fn file_info(file: *const u16, title: *mut u16, length: *mut c_int) {
    let track = requested(file, Duration::ZERO);
    let name = match &track {
        Some(track) => tracks::display(track),
        None => unsafe { wide::read(file) },
    };
    if !name.is_empty() {
        unsafe { wide::write(title, GETFILEINFO_TITLE_LENGTH, &name) };
    }
    if !length.is_null() {
        unsafe { *length = track.as_ref().map_or(-1, tracks::length_ms) };
    }
}

unsafe extern "C" fn info_box(file: *const u16, parent: HWND) -> c_int {
    if let Some(track) = requested(file, INFO_WAIT) {
        info::show(parent, &track);
    }
    INFOBOX_UNCHANGED
}

unsafe extern "C" fn is_our_file(file: *const u16) -> c_int {
    c_int::from(track_id(file).is_some())
}

unsafe extern "C" fn play(file: *const u16) -> c_int {
    match track_id(file) {
        Some(id) => {
            playback::play(id);
            0
        }
        None => -1,
    }
}

unsafe extern "C" fn pause() {
    playback::set_paused(true);
}

unsafe extern "C" fn unpause() {
    playback::set_paused(false);
}

unsafe extern "C" fn is_paused() -> c_int {
    c_int::from(playback::is_paused())
}

unsafe extern "C" fn stop() {
    playback::stop();
}

unsafe extern "C" fn length() -> c_int {
    playback::length_ms()
}

unsafe extern "C" fn output_time() -> c_int {
    let time = playback::output_time();
    prefetch::consider(time, playback::length_ms());
    time
}

unsafe extern "C" fn set_output_time(position: c_int) {
    playback::seek(position);
}

unsafe extern "C" fn set_volume(volume: c_int) {
    playback::set_volume(volume);
}

unsafe extern "C" fn set_pan(pan: c_int) {
    playback::set_pan(pan);
}

unsafe extern "C" fn eq_set(_on: c_int, _data: *mut c_char, _preamp: c_int) {}
