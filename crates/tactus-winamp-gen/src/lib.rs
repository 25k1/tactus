#![allow(non_snake_case, clippy::missing_safety_doc)]

mod actions;
mod browser;
mod login;
mod queue;
mod session;
mod settings;
mod wave;

use std::ffi::{CStr, c_int};
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use tactus_winamp::sdk::*;
use tactus_winamp::text::{plain, text};
use tactus_winamp::wide::{ansi, wide};
use tactus_winamp::{DESCRIPTION, Winamp};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const MENU_POSITION: u32 = 2;
const TICK_MS: u32 = 1000;

static PLUGIN: Exported<GeneralPlugin> = Exported::new(GeneralPlugin {
    version: GPPHDR_VER,
    description: DESCRIPTION.as_ptr().cast_mut().cast(),
    init: Some(init),
    config: Some(config),
    quit: Some(quit),
    parent: null_mut(),
    instance: null_mut(),
});

static COMMANDS: OnceLock<Vec<(u16, Action)>> = OnceLock::new();
static PREVIOUS: AtomicIsize = AtomicIsize::new(0);
static TIMER: AtomicUsize = AtomicUsize::new(0);
static HOTKEYS: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Open,
    MyWave,
    Like,
    Dislike,
    WaveFromCurrent,
    NowPlaying,
    Settings,
}

impl Action {
    const ALL: [Action; 7] = [
        Action::Open,
        Action::MyWave,
        Action::Like,
        Action::Dislike,
        Action::WaveFromCurrent,
        Action::NowPlaying,
        Action::Settings,
    ];

    fn label(self) -> String {
        text(match self {
            Action::Open => "winamp-open",
            Action::MyWave => "winamp-menu-my-wave",
            Action::Like => "winamp-like",
            Action::Dislike => "winamp-dislike-current",
            Action::WaveFromCurrent => "winamp-wave-current",
            Action::NowPlaying => "winamp-what-playing",
            Action::Settings => "winamp-settings-menu",
        })
    }

    fn hotkey(self) -> &'static CStr {
        match self {
            Action::Open => c"tactus_yandex_open",
            Action::MyWave => c"tactus_yandex_my_wave",
            Action::Like => c"tactus_yandex_like",
            Action::Dislike => c"tactus_yandex_dislike",
            Action::WaveFromCurrent => c"tactus_yandex_wave_from_track",
            Action::NowPlaying => c"tactus_yandex_now_playing",
            Action::Settings => c"tactus_yandex_settings",
        }
    }

    fn run(self) {
        match self {
            Action::Open => browser::show(),
            Action::MyWave => actions::my_wave(),
            Action::Like => actions::like_current(),
            Action::Dislike => actions::dislike_current(),
            Action::WaveFromCurrent => actions::wave_from_current(),
            Action::NowPlaying => actions::say_now_playing(),
            Action::Settings => settings::show(owner()),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn winampGetGeneralPurposePlugin() -> *mut GeneralPlugin {
    PLUGIN.get()
}

#[unsafe(no_mangle)]
pub extern "C" fn tactusShowSettings(owner: isize) {
    settings::show(owner as HWND);
}

fn owner() -> HWND {
    let active = unsafe { GetActiveWindow() };
    if active.is_null() { winamp().window() } else { active }
}

fn winamp() -> Winamp {
    Winamp::new(unsafe { (*PLUGIN.get()).parent })
}

unsafe extern "C" fn init() -> c_int {
    let winamp = winamp();
    let store = tactus_winamp::prepare(winamp);
    session::init(winamp, unsafe { (*PLUGIN.get()).instance }, store);
    let commands =
        COMMANDS.get_or_init(|| Action::ALL.iter().map(|action| (winamp.register_command(), *action)).collect());
    add_menu(winamp, commands);
    let previous = unsafe { SetWindowLongPtrW(winamp.window(), GWLP_WNDPROC, procedure as *const () as usize as _) };
    PREVIOUS.store(previous as isize, Ordering::Release);
    TIMER.store(unsafe { SetTimer(null_mut(), 0, TICK_MS, Some(tick)) }, Ordering::Release);
    if let Some(session) = session::get() {
        session.refresh(|| {});
    }
    GEN_INIT_SUCCESS
}

unsafe extern "C" fn config() {
    settings::show(owner());
}

unsafe extern "C" fn quit() {
    browser::close();
    let timer = TIMER.swap(0, Ordering::AcqRel);
    if timer != 0 {
        unsafe { KillTimer(null_mut(), timer) };
    }
    let previous = PREVIOUS.swap(0, Ordering::AcqRel);
    if previous != 0 {
        unsafe { SetWindowLongPtrW(winamp().window(), GWLP_WNDPROC, previous as _) };
    }
}

fn add_menu(winamp: Winamp, commands: &[(u16, Action)]) {
    let main = winamp.main_menu();
    if main.is_null() {
        return;
    }
    let submenu = unsafe { CreatePopupMenu() };
    for (id, action) in commands {
        let label = wide(&action.label());
        unsafe { AppendMenuW(submenu, MF_STRING, usize::from(*id), label.as_ptr()) };
    }
    let title = wide(&text("winamp-menu"));
    unsafe { InsertMenuW(main, MENU_POSITION, MF_BYPOSITION | MF_POPUP | MF_STRING, submenu as usize, title.as_ptr()) };
}

fn register_hotkeys(winamp: Winamp, commands: &[(u16, Action)]) {
    let message = winamp.register_message(c"GenHotkeysAdd");
    if message == 0 {
        return;
    }
    let prefix = text("winamp-title");
    for (id, action) in commands {
        let name = Box::leak(ansi(&format!("{prefix}: {}", plain(&action.label()))).into_boxed_slice());
        let hotkey = Box::leak(Box::new(HotkeyAdd {
            name: name.as_mut_ptr().cast(),
            flags: 0,
            message: WM_COMMAND,
            wparam: usize::from(*id),
            lparam: 0,
            id: action.hotkey().as_ptr().cast_mut(),
            window: null_mut(),
            extended: [0; 6],
        }));
        winamp.send(std::ptr::from_mut(hotkey) as usize, message);
    }
}

unsafe extern "system" fn tick(_: HWND, _: u32, _: usize, _: u32) {
    if !HOTKEYS.swap(true, Ordering::AcqRel)
        && let Some(commands) = COMMANDS.get()
    {
        register_hotkeys(winamp(), commands);
    }
    wave::tick();
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_COMMAND
        && let Some((_, action)) =
            COMMANDS.get().and_then(|commands| commands.iter().find(|(id, _)| usize::from(*id) == wparam & 0xffff))
    {
        action.run();
        return 0;
    }
    let previous: WNDPROC = unsafe { std::mem::transmute(PREVIOUS.load(Ordering::Acquire)) };
    unsafe { CallWindowProcW(previous, window, message, wparam, lparam) }
}
