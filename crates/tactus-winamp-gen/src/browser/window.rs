use super::Browser;
use super::form::{LIST, PLACES_LIST, SEARCH, SECTION, SETTINGS};
use crate::session::{self, Session};
use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, Once};
use tactus_api::sync::Guard;
use tactus_winamp::text::text;
use tactus_winamp::ui::controls;
use tactus_winamp::wide::wide;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::COLOR_BTNFACE;
use windows_sys::Win32::UI::Controls::{
    ICC_LISTVIEW_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, LVIS_SELECTED, LVN_GETDISPINFOW, LVN_ITEMCHANGED,
    LVN_KEYDOWN, LVN_ODFINDITEMW, NM_DBLCLK, NMHDR, NMLISTVIEW, NMLVDISPINFOW, NMLVFINDITEMW, NMLVKEYDOWN,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, IsWindowEnabled, VK_BACK, VK_CONTROL, VK_F5, VK_LEFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS: &str = "TactusYandexBrowser";
const WM_INBOX: u32 = WM_APP + 1;
const PLACE_TIMER: usize = 1;
const PLACE_DELAY: u32 = 400;
const WIDTH: i32 = 860;
const HEIGHT: i32 = 620;

type Update = Box<dyn FnOnce(&mut Browser) + Send>;

static WINDOW: AtomicIsize = AtomicIsize::new(0);
static INBOX: Mutex<Vec<Update>> = Mutex::new(Vec::new());
static CLASS_REGISTERED: Once = Once::new();

thread_local! {
    static BROWSER: RefCell<Option<Browser>> = const { RefCell::new(None) };
    static FOCUSED: Cell<HWND> = const { Cell::new(null_mut()) };
}

fn window() -> HWND {
    WINDOW.load(Ordering::Acquire) as HWND
}

pub fn show() {
    let window = window();
    if window.is_null() {
        let _ = std::thread::Builder::new().name("yandex-browser".into()).spawn(run);
        return;
    }
    unsafe {
        ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window);
    }
}

pub fn close() {
    let window = window();
    if !window.is_null() {
        unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
    }
}

pub fn deliver(update: impl FnOnce(&mut Browser) + Send + 'static) {
    let window = window();
    if window.is_null() {
        return;
    }
    INBOX.guard().push(Box::new(update));
    unsafe { PostMessageW(window, WM_INBOX, 0, 0) };
}

pub fn click(window: HWND, control: HWND) {
    let id = unsafe { GetDlgCtrlID(control) };
    if let Ok(id) = usize::try_from(id) {
        unsafe { PostMessageW(window, WM_COMMAND, id, control as isize) };
    }
}

pub fn cancel_place_timer(window: HWND) {
    unsafe { KillTimer(window, PLACE_TIMER) };
}

fn with_browser<T>(action: impl FnOnce(&mut Browser) -> T) -> Option<T> {
    BROWSER.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(action))
}

fn register(session: &Session) {
    CLASS_REGISTERED.call_once(|| {
        let controls =
            INITCOMMONCONTROLSEX { dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_LISTVIEW_CLASSES };
        unsafe { InitCommonControlsEx(&raw const controls) };
        let name = wide(CLASS);
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: session.instance(),
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
            hbrBackground: (COLOR_BTNFACE + 1) as usize as _,
            lpszClassName: name.as_ptr(),
            ..unsafe { std::mem::zeroed() }
        };
        unsafe { RegisterClassExW(&raw const class) };
        std::mem::forget(name);
    });
}

fn create(session: &Session) -> HWND {
    let (class, title) = (wide(CLASS), wide(&text("winamp-title")));
    unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_APPWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            controls::system_scale(WIDTH),
            controls::system_scale(HEIGHT),
            null_mut(),
            null_mut(),
            session.instance(),
            null(),
        )
    }
}

fn run() {
    let Some(session) = session::get() else {
        return;
    };
    register(session);
    INBOX.guard().clear();
    let window = create(session);
    if window.is_null() {
        return;
    }
    WINDOW.store(window as isize, Ordering::Release);
    BROWSER.with(|cell| *cell.borrow_mut() = Some(Browser::new(window, session)));
    with_browser(Browser::start);
    unsafe {
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
    }
    pump(window);
    WINDOW.store(0, Ordering::Release);
    FOCUSED.set(null_mut());
    BROWSER.with(|cell| cell.borrow_mut().take());
    INBOX.guard().clear();
}

fn pump(window: HWND) {
    let mut message: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&raw mut message, null_mut(), 0, 0) } > 0 {
        if shortcut(&message) || unsafe { IsDialogMessageW(window, &raw const message) } != 0 {
            continue;
        }
        unsafe {
            TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
}

fn remember_focus(window: HWND) {
    let focused = unsafe { GetFocus() };
    if !focused.is_null() && unsafe { IsChild(window, focused) } != 0 {
        FOCUSED.set(focused);
    }
}

fn focusable(window: HWND, control: HWND) -> bool {
    !control.is_null()
        && unsafe { IsChild(window, control) } != 0
        && unsafe { IsWindowVisible(control) } != 0
        && unsafe { IsWindowEnabled(control) } != 0
}

fn restore_focus(window: HWND) {
    let target = [FOCUSED.get(), unsafe { GetFocus() }]
        .into_iter()
        .find(|control| focusable(window, *control))
        .unwrap_or_else(|| unsafe { GetNextDlgTabItem(window, null_mut(), 0) });
    if !target.is_null() {
        controls::focus(target);
    }
}

fn pressed(key: u16) -> bool {
    (unsafe { GetKeyState(i32::from(key)) }) < 0
}

fn shortcut(message: &MSG) -> bool {
    let key = message.wParam as u16;
    match message.message {
        WM_KEYDOWN if key == u16::from(b'F') && pressed(VK_CONTROL) => with_browser(Browser::focus_search),
        WM_KEYDOWN if key == VK_F5 => with_browser(Browser::refresh),
        WM_SYSKEYDOWN if key == VK_LEFT => with_browser(Browser::back),
        _ => None,
    }
    .is_some()
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_ACTIVATE if wparam >> 16 == 0 => {
            if wparam & 0xffff == WA_INACTIVE as usize {
                remember_focus(window);
            } else {
                restore_focus(window);
            }
            0
        }
        WM_SETFOCUS => {
            restore_focus(window);
            0
        }
        WM_SIZE => {
            with_browser(|browser| browser.form.layout());
            0
        }
        WM_COMMAND => {
            command(window, (wparam & 0xffff) as u16, (wparam >> 16) as u32);
            0
        }
        WM_NOTIFY => notify(lparam as *mut NMHDR),
        WM_CONTEXTMENU => {
            context_menu(wparam as HWND, lparam);
            0
        }
        WM_TIMER if wparam == PLACE_TIMER => {
            cancel_place_timer(window);
            with_browser(Browser::open_selected_place);
            0
        }
        WM_INBOX => {
            let updates = std::mem::take(&mut *INBOX.guard());
            for update in updates {
                with_browser(update);
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn command(window: HWND, id: u16, code: u32) {
    match (id, code) {
        (1, _) => {
            with_browser(Browser::enter);
        }
        (2, _) => unsafe {
            DestroyWindow(window);
        },
        (PLACES_LIST, CBN_SELCHANGE) => unsafe {
            SetTimer(window, PLACE_TIMER, PLACE_DELAY, None);
        },
        (SECTION, CBN_SELCHANGE) => {
            with_browser(Browser::section_changed);
        }
        (SETTINGS, BN_CLICKED) => crate::settings::show(window),
        (_, BN_CLICKED) if id >= SEARCH => {
            with_browser(|browser| browser.button(id));
        }
        _ => {}
    }
}

fn notify(header: *mut NMHDR) -> LRESULT {
    let code = unsafe { &*header };
    if code.idFrom != usize::from(LIST) {
        return 0;
    }
    match code.code {
        LVN_GETDISPINFOW => controls::display_info(unsafe { &mut *header.cast::<NMLVDISPINFOW>() }),
        LVN_ODFINDITEMW => return controls::find_row(unsafe { &*header.cast::<NMLVFINDITEMW>() }),
        LVN_ITEMCHANGED => {
            let change = unsafe { &*header.cast::<NMLISTVIEW>() };
            if change.uNewState & LVIS_SELECTED != 0 {
                with_browser(|browser| browser.item_changed(change.iItem.max(0) as usize));
            }
        }
        NM_DBLCLK => {
            with_browser(Browser::activate);
        }
        LVN_KEYDOWN if unsafe { &*header.cast::<NMLVKEYDOWN>() }.wVKey == VK_BACK => {
            with_browser(Browser::back);
        }
        _ => {}
    }
    0
}

fn context_menu(control: HWND, position: LPARAM) {
    let Some(Some((owner, items, point))) = with_browser(|browser| browser.menu(control, position)) else {
        return;
    };
    if let Some(id) = controls::popup(owner, &items, point) {
        with_browser(|browser| browser.button(id));
    }
}
