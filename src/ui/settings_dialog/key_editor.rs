use super::*;

#[derive(Clone, Copy)]
pub(super) struct KeyEditor {
    list: ListCtrl,
    pub(super) change: Button,
    pub(super) clear: Button,
    pub(super) reset: Button,
}

impl KeyEditor {
    pub(super) fn new(panel: &Panel, sizer: &BoxSizer, name: &str) -> Self {
        add(sizer, &label(panel, name));
        let list = ListCtrl::builder(panel).with_style(ListCtrlStyle::Report | ListCtrlStyle::SingleSel).build();
        list.set_accessibility_label(name);
        list.insert_column(0, &fl!("column-action"), ListColumnFormat::Left, 300);
        list.insert_column(1, &fl!("column-shortcut"), ListColumnFormat::Left, 220);
        sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 6);
        let line = row();
        let [change, clear, reset] =
            [fl!("hotkey-change"), fl!("hotkey-clear"), fl!("hotkey-reset")].map(|text| button(panel, &text));
        for control in [&change, &clear, &reset] {
            line.add(control, 0, centered(), 6);
        }
        sizer.add_sizer(&line, 0, SizerFlag::All, 6);
        Self { list, change, clear, reset }
    }

    pub(super) fn fill(&self, rows: &[(String, Option<String>)]) {
        let selected = self.list.get_first_selected_item().max(0);
        self.list.delete_all_items();
        for (row, (action, key)) in rows.iter().enumerate() {
            self.list.insert_item(row as i64, action, None);
            let key = key.clone().filter(|key| !key.is_empty()).unwrap_or_else(|| fl!("hotkey-none"));
            self.list.set_item_text_by_column(row as i64, 1, &key);
        }
        let state = ListItemState::Selected | ListItemState::Focused;
        self.list.set_item_state(i64::from(selected), state, state);
    }

    pub(super) fn selected(&self) -> Option<usize> {
        usize::try_from(self.list.get_first_selected_item()).ok()
    }
}
