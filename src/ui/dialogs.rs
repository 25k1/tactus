use super::view_model;
use super::widgets::*;
use crate::i18n::fl;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wxdragon::prelude::*;

fn caption(text: &str) -> String {
    text.replace('&', "")
}

pub fn ask_text(parent: &Frame, message: &str, title: &str, default: &str) -> Option<String> {
    let dialog = TextEntryDialog::builder(parent, message, &caption(title)).with_default_value(default).build();
    if dialog.show_modal() != ID_OK {
        return None;
    }
    dialog.get_value().map(|value| value.trim().to_owned()).filter(|value| !value.is_empty())
}

pub fn confirm(parent: &Frame, message: &str, title: &str) -> bool {
    MessageDialog::builder(parent, message, &caption(title))
        .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion)
        .build()
        .show_modal()
        == ID_YES
}

pub fn warn(parent: &impl WxWidget, message: &str, title: &str) {
    MessageDialog::builder(parent, message, &caption(title))
        .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning)
        .build()
        .show_modal();
}

pub fn choose(parent: &Frame, message: &str, title: &str, items: &[String]) -> Option<usize> {
    let choices: Vec<&str> = items.iter().map(String::as_str).collect();
    let dialog = SingleChoiceDialog::builder(parent, message, &caption(title), &choices).build();
    (dialog.show_modal() == ID_OK).then(|| dialog.get_selection().max(0) as usize)
}

pub fn show_text(parent: &Frame, title: &str, text: &str) {
    let dialog = Dialog::builder(parent, &caption(title))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(520, 380)
        .build();
    let body = read_only_text(&dialog, &caption(title), -1);
    body.change_value(&view_model::windows_lines(text));
    let close = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&fl!("dialog-close")).build();
    let sizer = column();
    sizer.add(&body, 1, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 8);
    dialog.set_sizer(sizer, true);
    dialog.set_escape_id(ID_CANCEL);
    body.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

fn matches(text: &str, needle: &str) -> bool {
    let text = text.to_lowercase();
    needle.split_whitespace().all(|word| text.contains(word))
}

pub fn pick(parent: &Frame, title: &str, prompt: &str, items: Vec<String>) -> Option<usize> {
    let dialog = Dialog::builder(parent, &caption(title))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(520, 440)
        .build();
    let sizer = column();
    let filter = field(&sizer, &dialog, prompt, true, || {
        TextCtrl::builder(&dialog).with_style(TextCtrlStyle::ProcessEnter).build()
    });
    let list = ListBox::builder(&dialog).build();
    list.set_accessibility_label(&caption(title));
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 8);
    dialog.set_sizer(sizer, true);
    let items = Rc::new(items);
    let shown: Rc<RefCell<Vec<usize>>> = Rc::default();
    let chosen: Rc<Cell<Option<usize>>> = Rc::default();
    let refill = {
        let (items, shown) = (items.clone(), shown.clone());
        move || {
            let needle = filter.get_value().to_lowercase();
            let matching: Vec<usize> = (0..items.len()).filter(|index| matches(&items[*index], &needle)).collect();
            let labels: Vec<&str> = matching.iter().map(|index| items[*index].as_str()).collect();
            list.set_items(&labels);
            if !matching.is_empty() {
                list.set_selection(0, true);
            }
            *shown.borrow_mut() = matching;
        }
    };
    refill();
    filter.on_text_updated(move |_| refill());
    let choose = {
        let (shown, chosen) = (shown.clone(), chosen.clone());
        move || {
            if let Some(index) = list.get_selection().and_then(|row| shown.borrow().get(row as usize).copied()) {
                chosen.set(Some(index));
                dialog.end_modal(ID_OK);
            }
        }
    };
    {
        let choose = choose.clone();
        filter.on_enter_pressed(move |_| choose());
    }
    {
        let choose = choose.clone();
        list.on_item_double_clicked(move |_| choose());
    }
    WindowEvents::on_key_down(&filter, move |event| match event {
        WindowEventData::Keyboard(key) if key.get_key_code() == Some(WXK_DOWN) => list.set_focus(),
        _ => event.skip(true),
    });
    WindowEvents::on_key_down(&list, move |event| match event {
        WindowEventData::Keyboard(key) if key.get_key_code() == Some(WXK_RETURN) => choose(),
        _ => event.skip(true),
    });
    dialog.set_escape_id(ID_CANCEL);
    filter.set_focus();
    dialog.show_modal();
    dialog.destroy();
    chosen.get()
}
