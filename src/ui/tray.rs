use super::view_model;
use super::*;
use crate::i18n::fl;

const TOOLTIP_LIMIT: usize = 127;

impl Ui {
    pub(super) fn hide_window(&self) {
        if self.tray.borrow().is_some() {
            self.frame.show(false);
        } else {
            self.frame.iconize(true);
        }
    }

    pub(super) fn toggle_window(&self) {
        if self.frame.is_shown() && !self.frame.is_iconized() {
            self.hide_window();
        } else {
            self.raise_window();
        }
    }

    pub(super) fn raise_window(&self) {
        self.frame.show(true);
        if self.frame.is_iconized() {
            self.frame.iconize(false);
        }
        self.frame.raise();
        self.frame.set_focus();
        self.focus_content();
    }

    pub(super) fn set_tray_visible(&self, visible: bool) {
        if !visible && let Some(tray) = self.tray.borrow_mut().take() {
            tray.remove_icon();
            tray.destroy();
        }
    }

    pub(super) fn set_tray(self: &Rc<Self>, enabled: bool) {
        if !enabled {
            return self.set_tray_visible(false);
        }
        if self.tray.borrow().is_some() {
            return;
        }
        let mut builder = TaskBarIcon::builder().with_tooltip("Tactus");
        if let Some(icon) = app_icon(16) {
            builder = builder.with_icon(icon);
        }
        let tray = builder.build();
        let frame = self.frame;
        tray.on_menu(move |event| frame.post_menu_command(event.get_id()));
        tray.on_left_up(move |_| frame.post_menu_command(Command::ToggleWindow.id()));
        let ui = self.clone();
        tray.on_right_up(move |_| ui.show_tray_menu());
        *self.tray.borrow_mut() = Some(tray);
        self.update_tray_tooltip();
    }

    fn show_tray_menu(&self) {
        let (playing, title) = {
            let player = self.player.borrow();
            (player.playing, view_model::now_playing(&player).unwrap_or_else(|| fl!("now-playing-nothing")))
        };
        let visible = self.frame.is_shown() && !self.frame.is_iconized();
        let mut menu = Menu::builder()
            .append_item(ID_ANY as Id, &command::escape(&title), "")
            .append_separator()
            .append_item(Command::PlayPause.id(), &if playing { fl!("tray-pause") } else { fl!("tray-play") }, "")
            .append_item(Command::Next.id(), &fl!("tray-next"), "")
            .append_item(Command::Previous.id(), &fl!("tray-previous"), "")
            .append_item(Command::Like.id(), &fl!("tray-like"), "")
            .append_separator()
            .append_item(Command::ToggleWindow.id(), &if visible { fl!("tray-hide") } else { fl!("tray-show") }, "")
            .append_item(Command::Quit.id(), &fl!("tray-quit"), "")
            .build();
        if let Some(item) = menu.find_item_by_position(0) {
            item.enable(false);
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.popup_menu(&mut menu);
        }
    }

    pub(super) fn update_tray_tooltip(&self) {
        let tooltip = view_model::now_playing(&self.player.borrow()).unwrap_or_else(|| "Tactus".to_owned());
        if let (Some(tray), Some(icon)) = (self.tray.borrow().as_ref(), app_icon(16)) {
            let tooltip: String = tooltip.chars().take(TOOLTIP_LIMIT).collect();
            tray.set_icon(&icon, &tooltip);
        }
    }
}
