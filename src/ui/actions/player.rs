use super::*;
use crate::ui::bar::PlayerView;
use crate::ui::command::{Binding, PLAYER_MENU, command_menu};
use crate::ui::key_capture::KeyPress;
use crate::ui::mini_player::MiniPlayer;

impl Ui {
    pub(crate) fn each_player(&self, update: impl Fn(&dyn PlayerView, &Player)) {
        let player = self.player.borrow();
        update(&self.bar, &player);
        if let Some(mini) = self.mini.borrow().as_ref() {
            update(mini, &player);
        }
    }

    pub(super) fn toggle_player_controls(self: &Rc<Self>) {
        let compact = !self.player.borrow().settings.compact_player;
        self.act(|player| player.update_settings(|settings| settings.compact_player = compact));
        self.bar.set_compact(compact);
        self.bar.toggle.set_focus();
    }

    pub(super) fn show_player_menu(&self, window: &impl WxWidget) {
        let mut menu = command_menu(&self.keymap(), &PLAYER_MENU);
        menu.check_item(Command::Shuffle.id(), self.player.borrow().settings.shuffle);
        window.popup_menu(&mut menu, None);
    }

    pub(super) fn bind_player_menu(self: &Rc<Self>, panel: &Panel) {
        let (ui, target) = (self.clone(), *panel);
        panel.bind_internal(EventType::CONTEXT_MENU, move |_| ui.show_player_menu(&target));
    }

    pub(super) fn toggle_mini_player(self: &Rc<Self>) {
        let open = self.mini.borrow().as_ref().map(|mini| mini.frame);
        if let Some(frame) = open {
            frame.close(true);
            return;
        }
        let mini = MiniPlayer::new(&self.player.borrow());
        self.bind_mini_player(&mini);
        mini.frame.show(true);
        mini.play.set_focus();
        self.frame.show(false);
        *self.mini.borrow_mut() = Some(mini);
    }

    fn bind_mini_player(self: &Rc<Self>, mini: &MiniPlayer) {
        for (control, command) in [
            (&mini.previous, Command::Previous),
            (&mini.play, Command::PlayPause),
            (&mini.next, Command::Next),
            (&mini.slower, Command::SpeedDown),
            (&mini.faster, Command::SpeedUp),
            (&mini.full, Command::MiniPlayer),
        ] {
            let ui = self.clone();
            control.on_click(move |_| ui.execute(command));
        }
        let ui = self.clone();
        mini.volume.on_slider(move |event| {
            let volume = event.get_value() as i16;
            ui.act(|player| player.set_volume(volume));
        });
        self.bind_player_menu(&mini.panel);
        let ui = self.clone();
        mini.frame.on_menu(move |event| ui.on_menu(event.get_id()));
        let ui = self.clone();
        WindowEvents::on_char_hook(&mini.frame, move |event| {
            let WindowEventData::Keyboard(key) = &event else { return event.skip(true) };
            let code = key.get_key_code().unwrap_or_default();
            if code == WXK_ESCAPE {
                return ui.execute(Command::MiniPlayer);
            }
            let held = [key.control_down(), key.alt_down(), key.shift_down(), key.meta_down()];
            let shortcut = KeyPress::new(code, held).and_then(KeyPress::accelerator);
            match shortcut.and_then(|shortcut| ui.keymap().binding(&shortcut)) {
                Some(Binding::Command(command)) => ui.execute(command),
                _ => event.skip(true),
            }
        });
        let ui = self.clone();
        mini.frame.on_close(move |event| {
            ui.mini.borrow_mut().take();
            if !ui.quitting.get() {
                ui.raise_window();
            }
            event.skip(true);
        });
    }
}
