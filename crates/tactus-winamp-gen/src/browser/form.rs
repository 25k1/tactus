use std::ptr::null_mut;
use tactus_winamp::text::text;
use tactus_winamp::ui::controls;
use windows_sys::Win32::Foundation::HWND;

const MARGIN: i32 = 8;
const ROW: i32 = 24;
const LABEL: i32 = 120;
const SEARCH_WIDTH: i32 = 104;
const SIDE_WIDTH: i32 = 150;
const STATUS_HEIGHT: i32 = 52;
const BUTTONS_PER_ROW: i32 = 5;
const DROPDOWN_ROWS: i32 = 12;

pub const SEARCH_TEXT: u16 = 100;
pub const SEARCH: u16 = 101;
pub const PLACES_LIST: u16 = 102;
pub const SECTION: u16 = 103;
pub const LIST: u16 = 104;
pub const PLAY: u16 = 110;
pub const ENQUEUE: u16 = 111;
pub const NEXT: u16 = 112;
pub const OPEN: u16 = 113;
pub const RADIO: u16 = 114;
pub const LIKE: u16 = 115;
pub const DISLIKE: u16 = 116;
pub const MORE: u16 = 117;
pub const BACK: u16 = 118;
pub const WAVE: u16 = 119;
pub const SETTINGS: u16 = 121;
pub const STATUS: u16 = 122;

pub const ITEM_ACTIONS: [(u16, &str); 8] = [
    (PLAY, "winamp-play"),
    (ENQUEUE, "winamp-enqueue"),
    (NEXT, "winamp-play-next"),
    (OPEN, "winamp-open-item"),
    (RADIO, "winamp-radio"),
    (LIKE, "winamp-like-item"),
    (DISLIKE, "winamp-dislike"),
    (MORE, "winamp-more"),
];
const PAGE_ACTIONS: [(u16, &str); 2] = [(BACK, "winamp-back"), (WAVE, "winamp-start-wave")];

#[derive(Clone, Copy)]
pub struct Field {
    label: HWND,
    pub control: HWND,
}

impl Field {
    fn new(parent: HWND, key: &str, create: impl FnOnce(HWND) -> HWND) -> Self {
        let label = controls::label(parent, &text(key));
        Self { label, control: create(parent) }
    }

    fn beside(self, x: i32, y: i32, width: i32, height: i32) {
        controls::place(self.label, x, y + 4, LABEL, ROW);
        controls::place(self.control, x + LABEL + MARGIN, y, width, height);
    }

    fn below(self, x: i32, y: i32, width: i32, height: i32) {
        let caption = ROW - 4;
        controls::place(self.label, x, y, LABEL, caption - 2);
        controls::place(self.control, x, y + caption, width, height - caption);
    }
}

pub struct Form {
    pub window: HWND,
    pub search: Field,
    search_button: HWND,
    pub places: Field,
    pub section: Field,
    pub list: Field,
    buttons: Vec<(u16, HWND)>,
    pub status: Field,
    pub settings: HWND,
}

impl Form {
    pub fn new(window: HWND, places: &[String]) -> Self {
        let columns = [(text("column-title"), 320), (text("column-details"), 340), (text("column-duration"), 90)];
        let columns: Vec<(&str, i32)> = columns.iter().map(|(title, width)| (title.as_str(), *width)).collect();
        let form = Self {
            window,
            search: Field::new(window, "winamp-search-label", |parent| controls::edit(parent, SEARCH_TEXT)),
            search_button: controls::button(window, SEARCH, &text("winamp-search")),
            places: Field::new(window, "winamp-places", |parent| controls::combo(parent, PLACES_LIST)),
            section: Field::new(window, "winamp-section", |parent| controls::combo(parent, SECTION)),
            list: Field::new(window, "winamp-items", |parent| controls::list(parent, LIST, &columns)),
            buttons: ITEM_ACTIONS
                .iter()
                .chain(&PAGE_ACTIONS)
                .map(|(id, key)| (*id, controls::button(window, *id, &text(key))))
                .collect(),
            status: Field::new(window, "winamp-status", |parent| controls::read_only(parent, STATUS)),
            settings: controls::button(window, SETTINGS, &text("winamp-settings-button")),
        };
        controls::fill_combo(form.places.control, places, 0);
        form
    }

    pub fn button(&self, id: u16) -> HWND {
        self.buttons.iter().find(|(button, _)| *button == id).map_or(null_mut(), |(_, window)| *window)
    }

    pub fn layout(&self) {
        let (width, height) = controls::logical_client(self.window);
        let inner = width - MARGIN * 2;
        let span = inner - LABEL - MARGIN;
        let dropdown = ROW * DROPDOWN_ROWS;
        let mut y = MARGIN;
        self.search.beside(MARGIN, y, span - SEARCH_WIDTH - MARGIN, ROW);
        controls::place(self.search_button, width - MARGIN - SEARCH_WIDTH, y, SEARCH_WIDTH, ROW);
        y += ROW + MARGIN;
        self.places.beside(MARGIN, y, span, dropdown);
        y += ROW + MARGIN;
        self.section.beside(MARGIN, y, span, dropdown);
        y += ROW + MARGIN;
        let rows = self.buttons.len().div_ceil(BUTTONS_PER_ROW as usize) as i32;
        let footer = rows * (ROW + 4) + MARGIN * 2 + STATUS_HEIGHT;
        let mut bottom = height - footer;
        self.list.below(MARGIN, y, inner, bottom - y - MARGIN);
        let button_width = (inner - MARGIN * (BUTTONS_PER_ROW - 1)) / BUTTONS_PER_ROW;
        for (index, (_, button)) in self.buttons.iter().enumerate() {
            let (column, row) = (index as i32 % BUTTONS_PER_ROW, index as i32 / BUTTONS_PER_ROW);
            let x = MARGIN + column * (button_width + MARGIN);
            controls::place(*button, x, bottom + row * (ROW + 4), button_width, ROW);
        }
        bottom += rows * (ROW + 4) + MARGIN;
        let right = width - MARGIN - SIDE_WIDTH;
        self.status.beside(MARGIN, bottom, right - LABEL - MARGIN * 3, STATUS_HEIGHT);
        controls::place(self.settings, right, bottom, SIDE_WIDTH, ROW);
    }
}
