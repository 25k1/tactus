use crate::wide::{read, wide, write};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use tactus_api::sync::Guard;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, CreateFontIndirectW, DEFAULT_GUI_FONT, GetStockObject, HFONT};
use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const SS_LEFT: u32 = 0;
const STANDARD_DPI: i32 = 96;

thread_local! {
    static ROWS: RefCell<Vec<[String; 3]>> = const { RefCell::new(Vec::new()) };
}

pub fn create(parent: HWND, class: &str, text: &str, style: u32, id: u16) -> HWND {
    let (class, text) = (wide(class), wide(text));
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            parent,
            id as usize as HMENU,
            null_mut(),
            null(),
        )
    };
    unsafe { SendMessageW(window, WM_SETFONT, font(dpi(parent)) as usize, 1) };
    window
}

pub fn label(parent: HWND, text: &str) -> HWND {
    create(parent, "STATIC", text, SS_LEFT, 0)
}

pub fn edit(parent: HWND, id: u16) -> HWND {
    create(parent, "EDIT", "", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32, id)
}

pub fn read_only(parent: HWND, id: u16) -> HWND {
    let style = WS_TABSTOP | WS_BORDER | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32;
    let window = create(parent, "EDIT", "", style, id);
    unsafe { SetWindowSubclass(window, Some(dialog_keys), 1, 0) };
    window
}

unsafe extern "system" fn dialog_keys(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let result = unsafe { DefSubclassProc(window, message, wparam, lparam) };
    if message == WM_GETDLGCODE { result & !((DLGC_WANTALLKEYS | DLGC_WANTTAB) as LRESULT) } else { result }
}

pub fn read_only_line(parent: HWND, id: u16) -> HWND {
    create(parent, "EDIT", "", WS_TABSTOP | WS_BORDER | (ES_READONLY | ES_AUTOHSCROLL) as u32, id)
}

pub fn button(parent: HWND, id: u16, text: &str) -> HWND {
    create(parent, "BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON as u32, id)
}

pub fn combo(parent: HWND, id: u16) -> HWND {
    create(parent, "COMBOBOX", "", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32, id)
}

pub fn list(parent: HWND, id: u16, columns: &[(&str, i32)]) -> HWND {
    let style = WS_TABSTOP | WS_BORDER | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | LVS_OWNERDATA;
    let window = create(parent, "SysListView32", "", style, id);
    unsafe { SendMessageW(window, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, LVS_EX_FULLROWSELECT as isize) };
    for (index, (title, width)) in columns.iter().enumerate() {
        let title = wide(title);
        let mut column: LVCOLUMNW = unsafe { std::mem::zeroed() };
        column.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
        column.cx = *width;
        column.pszText = title.as_ptr().cast_mut();
        column.iSubItem = index as i32;
        unsafe { SendMessageW(window, LVM_INSERTCOLUMNW, index, &raw const column as isize) };
    }
    window
}

pub fn set_text(window: HWND, text: &str) {
    let text = wide(text);
    unsafe { SetWindowTextW(window, text.as_ptr()) };
}

pub fn text(window: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(window) }.max(0) as usize;
    let mut buffer = vec![0u16; length + 1];
    let copied = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buffer[..copied])
}

pub fn fill_combo(window: HWND, items: &[String], selected: usize) {
    unsafe { SendMessageW(window, CB_RESETCONTENT, 0, 0) };
    for item in items {
        let item = wide(item);
        unsafe { SendMessageW(window, CB_ADDSTRING, 0, item.as_ptr() as isize) };
    }
    if !items.is_empty() {
        select(window, selected.min(items.len() - 1));
    }
}

pub fn select(window: HWND, index: usize) {
    unsafe { SendMessageW(window, CB_SETCURSEL, index, 0) };
}

pub fn combo_selection(window: HWND) -> Option<usize> {
    usize::try_from(unsafe { SendMessageW(window, CB_GETCURSEL, 0, 0) }).ok()
}

pub fn fill_list(window: HWND, rows: Vec<[String; 3]>, selected: usize) {
    let count = rows.len();
    ROWS.with(|cell| *cell.borrow_mut() = rows);
    unsafe {
        SendMessageW(window, LVM_DELETEALLITEMS, 0, 0);
        SendMessageW(window, LVM_SETITEMCOUNT, count, 0);
    }
    if count > 0 {
        select_row(window, selected.min(count - 1));
    }
}

pub fn display_info(info: &mut NMLVDISPINFOW) {
    let item = &mut info.item;
    if item.mask & LVIF_TEXT == 0 {
        return;
    }
    let (Ok(row), Ok(column), Ok(capacity)) =
        (usize::try_from(item.iItem), usize::try_from(item.iSubItem), usize::try_from(item.cchTextMax))
    else {
        return;
    };
    ROWS.with(|cell| {
        let rows = cell.borrow();
        let value = rows.get(row).and_then(|row| row.get(column)).map_or("", String::as_str);
        unsafe { write(item.pszText, capacity, value) };
    });
}

pub fn find_row(find: &NMLVFINDITEMW) -> isize {
    if find.lvfi.flags & (LVFI_STRING | LVFI_PARTIAL) == 0 {
        return -1;
    }
    let wanted = unsafe { read(find.lvfi.psz) }.to_lowercase();
    if wanted.is_empty() {
        return -1;
    }
    ROWS.with(|cell| {
        let rows = cell.borrow();
        let start = usize::try_from(find.iStart).unwrap_or(0).min(rows.len());
        (start..rows.len())
            .chain(0..start)
            .find(|index| rows[*index][0].to_lowercase().starts_with(&wanted))
            .map_or(-1, |index| index as isize)
    })
}

pub fn select_row(window: HWND, index: usize) {
    let mut item: LVITEMW = unsafe { std::mem::zeroed() };
    item.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
    item.state = LVIS_SELECTED | LVIS_FOCUSED;
    unsafe {
        SendMessageW(window, LVM_SETITEMSTATE, index, &raw const item as isize);
        SendMessageW(window, LVM_ENSUREVISIBLE, index, 0);
    }
}

pub fn row_point(window: HWND, index: usize) -> (i32, i32) {
    let mut rect = RECT { left: LVIR_BOUNDS as i32, top: 0, right: 0, bottom: 0 };
    unsafe { SendMessageW(window, LVM_GETITEMRECT, index, &raw mut rect as isize) };
    let mut point = windows_sys::Win32::Foundation::POINT { x: rect.left + 16, y: rect.bottom };
    unsafe { ClientToScreen(window, &raw mut point) };
    (point.x, point.y)
}

pub fn dpi(window: HWND) -> i32 {
    match unsafe { GetDpiForWindow(window) } {
        0 => STANDARD_DPI,
        dpi => dpi as i32,
    }
}

pub fn system_scale(value: i32) -> i32 {
    scale(value, unsafe { GetDpiForSystem() } as i32)
}

fn scale(value: i32, dpi: i32) -> i32 {
    value * dpi / STANDARD_DPI
}

pub fn logical_client(window: HWND) -> (i32, i32) {
    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(window, &raw mut rect) };
    let dpi = dpi(window);
    (rect.right * STANDARD_DPI / dpi, rect.bottom * STANDARD_DPI / dpi)
}

pub fn place(window: HWND, x: i32, y: i32, width: i32, height: i32) {
    let dpi = dpi(window);
    let [x, y, width, height] = [x, y, width.max(1), height.max(1)].map(|value| scale(value, dpi));
    unsafe { MoveWindow(window, x, y, width, height, 1) };
}

fn font(dpi: i32) -> HFONT {
    static FONTS: Mutex<Vec<(i32, isize)>> = Mutex::new(Vec::new());
    let mut fonts = FONTS.guard();
    if let Some((_, font)) = fonts.iter().find(|(known, _)| *known == dpi) {
        return *font as HFONT;
    }
    let mut metrics: NONCLIENTMETRICSW = unsafe { std::mem::zeroed() };
    metrics.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
    let size = metrics.cbSize;
    let found =
        unsafe { SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, size, (&raw mut metrics).cast(), 0, dpi as u32) };
    let font = if found != 0 {
        unsafe { CreateFontIndirectW(&raw const metrics.lfMessageFont) }
    } else {
        unsafe { GetStockObject(DEFAULT_GUI_FONT) }
    };
    fonts.push((dpi, font as isize));
    font
}

pub fn enable(window: HWND, enabled: bool) {
    unsafe { EnableWindow(window, i32::from(enabled)) };
}

pub fn show(window: HWND, visible: bool) {
    unsafe { ShowWindow(window, if visible { SW_SHOW } else { SW_HIDE }) };
}

pub fn focus(window: HWND) {
    unsafe { SetFocus(window) };
}

pub fn popup(owner: HWND, items: &[(u16, String)], (x, y): (i32, i32)) -> Option<u16> {
    let menu = unsafe { CreatePopupMenu() };
    for (id, label) in items {
        let label = wide(label);
        unsafe { AppendMenuW(menu, MF_STRING, usize::from(*id), label.as_ptr()) };
    }
    let chosen = unsafe { TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, 0, owner, null()) };
    unsafe { DestroyMenu(menu) };
    u16::try_from(chosen).ok().filter(|id| *id != 0)
}

pub fn copy(owner: HWND, text: &str) {
    let text = wide(text);
    unsafe {
        if OpenClipboard(owner) == 0 {
            return;
        }
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * 2);
        if !memory.is_null() {
            let target = GlobalLock(memory).cast::<u16>();
            if !target.is_null() {
                std::ptr::copy_nonoverlapping(text.as_ptr(), target, text.len());
                GlobalUnlock(memory);
                SetClipboardData(u32::from(CF_UNICODETEXT), memory);
            }
        }
        CloseClipboard();
    }
}

pub fn open_url(url: &str) {
    let (verb, url) = (wide("open"), wide(url));
    unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL) };
}
