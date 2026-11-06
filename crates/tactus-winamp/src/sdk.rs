use std::cell::UnsafeCell;
use std::ffi::{c_char, c_int, c_void};
use windows_sys::Win32::Foundation::{HINSTANCE, HWND};

pub const IN_VER: c_int = 0x0F00_0000 | 0x100;
pub const IN_MODULE_FLAG_USES_OUTPUT_PLUGIN: c_int = 1;
pub const GETFILEINFO_TITLE_LENGTH: usize = 2048;
pub const INFOBOX_UNCHANGED: c_int = 1;
pub const GPPHDR_VER: c_int = 0x10;
pub const GEN_INIT_SUCCESS: c_int = 0;

pub const WM_WA_IPC: u32 = 0x0400;
pub const WM_WA_MPEG_EOF: u32 = 0x0400 + 2;
pub const IPC_DELETE: u32 = 101;
pub const IPC_ISPLAYING: u32 = 104;
pub const IPC_GETOUTPUTTIME: u32 = 105;
pub const IPC_SETPLAYLISTPOS: u32 = 121;
pub const IPC_GETLISTLENGTH: u32 = 124;
pub const IPC_GETLISTPOS: u32 = 125;
pub const IPC_GETPLAYLISTTITLEW: u32 = 213;
pub const IPC_GETPLAYLISTFILEW: u32 = 214;
pub const IPC_GET_SHUFFLE: u32 = 250;
pub const IPC_GET_REPEAT: u32 = 251;
pub const IPC_GETWND: u32 = 260;
pub const IPC_GETWND_PE: usize = 1;
pub const IPC_GET_HMENU: u32 = 281;
pub const IPC_GETINIDIRECTORY: u32 = 335;
pub const IPC_PLAYFILEW: usize = 1100;
pub const IPC_REGISTER_LOWORD_COMMAND: u32 = 3019;
pub const IPC_GET_PROXY_STRING: u32 = 3023;
pub const IPC_REGISTER_WINAMP_IPCMESSAGE: u32 = 65536;
pub const IPC_PE_INSERTFILENAMEW: usize = 114;
pub const MAX_PATH: usize = 260;

pub const COMMAND_PLAY: usize = 40045;
pub const COMMAND_STOP: usize = 40047;
pub const COMMAND_NEXT: usize = 40048;

pub type Callback = Option<unsafe extern "C" fn()>;
pub type WindowCallback = Option<unsafe extern "C" fn(HWND)>;

#[repr(C)]
pub struct OutModule {
    pub version: c_int,
    pub description: *mut c_char,
    pub id: isize,
    pub main_window: HWND,
    pub instance: HINSTANCE,
    pub config: WindowCallback,
    pub about: WindowCallback,
    pub init: Callback,
    pub quit: Callback,
    pub open: Option<unsafe extern "C" fn(c_int, c_int, c_int, c_int, c_int) -> c_int>,
    pub close: Callback,
    pub write: Option<unsafe extern "C" fn(*mut c_char, c_int) -> c_int>,
    pub can_write: Option<unsafe extern "C" fn() -> c_int>,
    pub is_playing: Option<unsafe extern "C" fn() -> c_int>,
    pub pause: Option<unsafe extern "C" fn(c_int) -> c_int>,
    pub set_volume: Option<unsafe extern "C" fn(c_int)>,
    pub set_pan: Option<unsafe extern "C" fn(c_int)>,
    pub flush: Option<unsafe extern "C" fn(c_int)>,
    pub output_time: Option<unsafe extern "C" fn() -> c_int>,
    pub written_time: Option<unsafe extern "C" fn() -> c_int>,
}

#[repr(C)]
pub struct InModule {
    pub version: c_int,
    pub description: *mut c_char,
    pub main_window: HWND,
    pub instance: HINSTANCE,
    pub extensions: *mut c_char,
    pub seekable: c_int,
    pub flags: c_int,
    pub config: WindowCallback,
    pub about: WindowCallback,
    pub init: Callback,
    pub quit: Callback,
    pub file_info: Option<unsafe extern "C" fn(*const u16, *mut u16, *mut c_int)>,
    pub info_box: Option<unsafe extern "C" fn(*const u16, HWND) -> c_int>,
    pub is_our_file: Option<unsafe extern "C" fn(*const u16) -> c_int>,
    pub play: Option<unsafe extern "C" fn(*const u16) -> c_int>,
    pub pause: Callback,
    pub unpause: Callback,
    pub is_paused: Option<unsafe extern "C" fn() -> c_int>,
    pub stop: Callback,
    pub length: Option<unsafe extern "C" fn() -> c_int>,
    pub output_time: Option<unsafe extern "C" fn() -> c_int>,
    pub set_output_time: Option<unsafe extern "C" fn(c_int)>,
    pub set_volume: Option<unsafe extern "C" fn(c_int)>,
    pub set_pan: Option<unsafe extern "C" fn(c_int)>,
    pub sa_vsa_init: Option<unsafe extern "C" fn(c_int, c_int)>,
    pub sa_vsa_deinit: Callback,
    pub sa_add_pcm: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int)>,
    pub sa_get_mode: Option<unsafe extern "C" fn() -> c_int>,
    pub sa_add: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int>,
    pub vsa_add_pcm: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int)>,
    pub vsa_get_mode: Option<unsafe extern "C" fn(*mut c_int, *mut c_int) -> c_int>,
    pub vsa_add: Option<unsafe extern "C" fn(*mut c_void, c_int) -> c_int>,
    pub vsa_set_info: Option<unsafe extern "C" fn(c_int, c_int)>,
    pub dsp_is_active: Option<unsafe extern "C" fn() -> c_int>,
    pub dsp_do_samples: Option<unsafe extern "C" fn(*mut i16, c_int, c_int, c_int, c_int) -> c_int>,
    pub eq_set: Option<unsafe extern "C" fn(c_int, *mut c_char, c_int)>,
    pub set_info: Option<unsafe extern "C" fn(c_int, c_int, c_int, c_int)>,
    pub out: *mut OutModule,
    pub service: *mut c_void,
    pub reserved: [*mut c_void; 32],
}

#[repr(C)]
pub struct GeneralPlugin {
    pub version: c_int,
    pub description: *mut c_char,
    pub init: Option<unsafe extern "C" fn() -> c_int>,
    pub config: Callback,
    pub quit: Callback,
    pub parent: HWND,
    pub instance: HINSTANCE,
}

#[repr(C)]
pub struct FileInfoW {
    pub file: [u16; MAX_PATH],
    pub index: c_int,
}

#[repr(C)]
pub struct HotkeyAdd {
    pub name: *mut c_char,
    pub flags: u32,
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub id: *mut c_char,
    pub window: HWND,
    pub extended: [c_int; 6],
}

pub struct Exported<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Exported<T> {}

impl<T> Exported<T> {
    pub const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    pub const fn get(&self) -> *mut T {
        self.0.get()
    }
}
