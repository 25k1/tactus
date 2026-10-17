use wxdragon::prelude::*;

pub fn label(parent: &impl WxWidget, text: &str) -> StaticText {
    StaticText::builder(parent).with_label(text).build()
}

pub fn button(parent: &impl WxWidget, text: &str) -> Button {
    Button::builder(parent).with_label(text).build()
}

pub fn check(parent: &impl WxWidget, text: &str, value: bool) -> CheckBox {
    CheckBox::builder(parent).with_label(text).with_value(value).build()
}

pub fn choice(parent: &impl WxWidget, items: &[String], selection: usize) -> Choice {
    Choice::builder(parent)
        .with_choices(items.to_vec())
        .with_selection(Some(selection.min(items.len().saturating_sub(1)) as u32))
        .build()
}

pub fn read_only_text(parent: &impl WxWidget, name: &str, height: i32) -> TextCtrl {
    let text = TextCtrl::builder(parent)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::WordWrap)
        .with_size(Size::new(-1, height))
        .build();
    text.set_accessibility_label(name);
    text
}

pub fn row() -> BoxSizer {
    BoxSizer::builder(Orientation::Horizontal).build()
}

pub fn column() -> BoxSizer {
    BoxSizer::builder(Orientation::Vertical).build()
}

pub fn centered() -> SizerFlag {
    SizerFlag::AlignCenterVertical | SizerFlag::Right
}

pub fn labeled_field<W: WxWidget>(
    sizer: &BoxSizer,
    parent: &impl WxWidget,
    text: &str,
    expand: bool,
    make: impl FnOnce() -> W,
) -> (StaticText, W) {
    let line = row();
    let caption = label(parent, text);
    line.add(&caption, 0, centered(), 8);
    let control = make();
    line.add(&control, i32::from(expand), SizerFlag::AlignCenterVertical, 0);
    let flags = if expand { SizerFlag::Expand | SizerFlag::All } else { SizerFlag::All };
    sizer.add_sizer(&line, 0, flags, 6);
    (caption, control)
}

pub fn field<W: WxWidget>(
    sizer: &BoxSizer,
    parent: &impl WxWidget,
    text: &str,
    expand: bool,
    make: impl FnOnce() -> W,
) -> W {
    labeled_field(sizer, parent, text, expand, make).1
}

pub fn selection(choice: &Choice) -> usize {
    choice.get_selection().unwrap_or(0) as usize
}

pub fn set_choices(choice: &Choice, items: &[String], selected: usize) {
    choice.clear();
    for item in items {
        choice.append(item);
    }
    if !items.is_empty() {
        choice.set_selection(selected.min(items.len() - 1) as u32);
    }
}

pub fn app_icon(size: i32) -> Option<Bitmap> {
    BitmapBundle::from_svg_data(include_bytes!("../../assets/tactus.svg"), Size::new(size, size))?
        .get_bitmap(Size::new(size, size))
}
