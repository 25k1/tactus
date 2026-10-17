use super::*;
use tactus_core::settings::Column;

type Model = Rc<RefCell<Vec<(Column, bool)>>>;

pub(super) struct Lists {
    columns: CheckListBox,
    model: Model,
}

fn column_label(column: Column) -> String {
    match column {
        Column::Title => fl!("column-title"),
        Column::Artist => fl!("column-artist"),
        Column::Details => fl!("column-details"),
        Column::Duration => fl!("column-duration"),
    }
}

fn sync(columns: &CheckListBox, model: &Model) {
    for (index, entry) in model.borrow_mut().iter_mut().enumerate() {
        entry.1 = columns.is_checked(index as u32);
    }
}

fn show(columns: &CheckListBox, model: &Model, selected: usize) {
    columns.clear();
    for (index, (column, shown)) in model.borrow().iter().enumerate() {
        columns.append(&column_label(*column));
        columns.check(index as u32, *shown);
    }
    columns.set_selection(selected as u32, true);
}

impl Lists {
    pub(super) fn new(notebook: &Notebook, old: &Settings) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-lists"));
        add(&sizer, &label(&panel, &fl!("settings-columns")));
        let columns = CheckListBox::builder(&panel).build();
        columns.set_accessibility_label(&fl!("settings-columns"));
        sizer.add(&columns, 1, SizerFlag::Expand | SizerFlag::All, 6);
        let line = row();
        let [up, down] = [fl!("settings-move-up"), fl!("settings-move-down")].map(|text| button(&panel, &text));
        line.add(&up, 0, centered(), 6);
        line.add(&down, 0, centered(), 6);
        sizer.add_sizer(&line, 0, SizerFlag::All, 6);
        add(&sizer, &label(&panel, &fl!("settings-columns-hint")));
        panel.set_sizer(sizer, true);
        let model: Model = Rc::new(RefCell::new(
            old.columns
                .iter()
                .map(|column| (*column, true))
                .chain(
                    Column::ALL
                        .into_iter()
                        .filter(|column| !old.columns.contains(column))
                        .map(|column| (column, false)),
                )
                .collect(),
        ));
        show(&columns, &model, 0);
        for (control, step) in [(up, -1isize), (down, 1)] {
            let model = model.clone();
            control.on_click(move |_| {
                let Some(from) = columns.get_selection().map(|index| index as usize) else { return };
                let Some(to) = from.checked_add_signed(step).filter(|to| *to < model.borrow().len()) else { return };
                sync(&columns, &model);
                model.borrow_mut().swap(from, to);
                show(&columns, &model, to);
                columns.set_focus();
            });
        }
        Self { columns, model }
    }
}

impl Tab for Lists {
    fn apply(&self, settings: &mut Settings) {
        sync(&self.columns, &self.model);
        let chosen: Vec<Column> =
            self.model.borrow().iter().filter(|(_, shown)| *shown).map(|(column, _)| *column).collect();
        settings.columns = if chosen.is_empty() { vec![Column::Title] } else { chosen };
    }
}
