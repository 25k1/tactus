use super::view_model::{Entry, Row, columns};
use crate::i18n::fl;
use std::cell::RefCell;
use tactus_api::Track;
use tactus_core::settings::Column;
use wxdragon::prelude::*;

pub struct MediaList {
    pub list: ListCtrl,
    rows: RefCell<Vec<Row>>,
    marked: RefCell<Option<usize>>,
    columns: RefCell<Vec<Column>>,
}

fn heading(column: Column) -> (String, i32, ListColumnFormat) {
    match column {
        Column::Title => (fl!("column-title"), 340, ListColumnFormat::Left),
        Column::Artist => (fl!("column-artist"), 220, ListColumnFormat::Left),
        Column::Details => (fl!("column-details"), 280, ListColumnFormat::Left),
        Column::Duration => (fl!("column-duration"), 80, ListColumnFormat::Right),
    }
}

impl MediaList {
    pub fn new(parent: &impl WxWidget, name: &str, columns: &[Column]) -> Self {
        let list = ListCtrl::builder(parent).with_style(ListCtrlStyle::Report).build();
        list.set_accessibility_label(name);
        let media = Self {
            list,
            rows: RefCell::new(Vec::new()),
            marked: RefCell::new(None),
            columns: RefCell::new(Vec::new()),
        };
        media.set_columns(columns);
        media
    }

    pub fn set_columns(&self, columns: &[Column]) {
        let columns = if columns.is_empty() { &[Column::Title][..] } else { columns };
        if *self.columns.borrow() == columns {
            return;
        }
        self.list.clear_all();
        for (index, column) in columns.iter().enumerate() {
            let (text, width, format) = heading(*column);
            self.list.insert_column(index as i64, &text, format, width);
        }
        *self.columns.borrow_mut() = columns.to_vec();
        let (rows, marked) = (self.rows.take(), self.marked.take());
        self.set(rows, marked, None);
    }

    fn write_row(&self, row: i64, entry: &Entry, insert: bool, marked: bool) {
        let texts = columns(entry);
        for (index, column) in self.columns.borrow().iter().enumerate() {
            let mut text = texts[*column as usize].clone();
            if index == 0 && marked {
                text = fl!("queue-current", title = text);
            }
            match index {
                0 if insert => {
                    self.list.insert_item(row, &text, None);
                }
                0 => self.list.set_item_text(row, &text),
                _ => self.list.set_item_text_by_column(row, index as i32, &text),
            }
        }
    }

    pub fn set(&self, rows: Vec<Row>, marked: Option<usize>, select: Option<usize>) {
        let previous = self.selected();
        let rebuild = rows.len() != self.rows.borrow().len() || self.list.get_item_count() as usize != rows.len();
        let changed = rebuild || *self.rows.borrow() != rows || *self.marked.borrow() != marked;
        if changed {
            if rebuild {
                self.list.freeze();
                self.list.delete_all_items();
            }
            for (index, row) in rows.iter().enumerate() {
                self.write_row(index as i64, &row.entry, rebuild, marked == Some(row.origin));
            }
            if rebuild {
                self.list.thaw();
            }
            *self.rows.borrow_mut() = rows;
            *self.marked.borrow_mut() = marked;
        }
        let count = self.len();
        let target = select.or_else(|| rebuild.then(|| previous.unwrap_or(0)));
        if let Some(row) = target.filter(|_| count > 0) {
            self.select(row.min(count - 1));
        }
    }

    fn next(&self, after: i64, state: ListItemState) -> Option<usize> {
        usize::try_from(self.list.get_next_item(after, ListNextItemFlag::All, state)).ok()
    }

    pub fn select(&self, row: usize) {
        for selected in self.selection() {
            self.list.set_item_state(selected as i64, ListItemState::None, ListItemState::Selected);
        }
        let state = ListItemState::Selected | ListItemState::Focused;
        self.list.set_item_state(row as i64, state, state);
        self.list.ensure_visible(row as i64);
    }

    pub fn select_all(&self) {
        for row in 0..self.len() {
            self.list.set_item_state(row as i64, ListItemState::Selected, ListItemState::Selected);
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.next(-1, ListItemState::Focused)
            .filter(|row| self.list.get_item_state(*row as i64, ListItemState::Selected))
            .or_else(|| self.next(-1, ListItemState::Selected))
    }

    pub fn selection(&self) -> Vec<usize> {
        std::iter::successors(self.next(-1, ListItemState::Selected), |row| {
            self.next(*row as i64, ListItemState::Selected)
        })
        .collect()
    }

    pub fn entry(&self, row: usize) -> Option<Entry> {
        self.rows.borrow().get(row).map(|row| row.entry.clone())
    }

    pub fn entries(&self, rows: &[usize]) -> Vec<(usize, Entry)> {
        let all = self.rows.borrow();
        rows.iter().filter_map(|row| all.get(*row)).map(|row| (row.origin, row.entry.clone())).collect()
    }

    pub fn origin(&self, row: usize) -> Option<usize> {
        self.rows.borrow().get(row).map(|row| row.origin)
    }

    pub fn len(&self) -> usize {
        self.rows.borrow().len()
    }

    pub fn tracks_from(&self, row: usize) -> (Vec<Track>, usize) {
        let rows = self.rows.borrow();
        let start = rows.iter().take(row).filter(|row| row.entry.track().is_some()).count();
        (rows.iter().filter_map(|row| row.entry.track()).cloned().collect(), start)
    }
}
