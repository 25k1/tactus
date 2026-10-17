use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tactus_core::hotkey::{GlobalAction, Hotkey};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_HOTKEY, WM_QUIT, WM_USER,
};

const RELEASE_POLL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed(GlobalAction),
    Released(GlobalAction),
}

fn watch_release(action: GlobalAction, key: u32, events: Sender<HotkeyEvent>) {
    std::thread::spawn(move || {
        while unsafe { GetAsyncKeyState(key as i32) } < 0 {
            std::thread::sleep(RELEASE_POLL);
        }
        let _ = events.send(HotkeyEvent::Released(action));
    });
}

fn flags(action: GlobalAction) -> u32 {
    if action.repeats() { 0 } else { MOD_NOREPEAT }
}

pub struct Hotkeys {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Hotkeys {
    pub fn start(bindings: Vec<(GlobalAction, Hotkey)>, events: Sender<HotkeyEvent>) -> (Hotkeys, Vec<GlobalAction>) {
        let (ready, started) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut message: MSG = unsafe { std::mem::zeroed() };
            unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE) };
            let failed: Vec<GlobalAction> = bindings
                .iter()
                .enumerate()
                .filter(|(index, (action, hotkey))| unsafe {
                    RegisterHotKey(
                        std::ptr::null_mut(),
                        *index as i32 + 1,
                        hotkey.modifiers | flags(*action),
                        hotkey.key,
                    ) == 0
                })
                .map(|(_, (action, _))| *action)
                .collect();
            let _ = ready.send((unsafe { GetCurrentThreadId() }, failed));
            while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
                if message.message == WM_HOTKEY
                    && let Some((action, hotkey)) = bindings.get(message.wParam.wrapping_sub(1))
                {
                    let _ = events.send(HotkeyEvent::Pressed(*action));
                    if *action == GlobalAction::Voice {
                        watch_release(*action, hotkey.key, events.clone());
                    }
                }
            }
            for index in 0..bindings.len() {
                unsafe { UnregisterHotKey(std::ptr::null_mut(), index as i32 + 1) };
            }
        });
        let (thread_id, failed) = started.recv().unwrap_or_default();
        (Hotkeys { thread_id, thread: Some(thread) }, failed)
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
