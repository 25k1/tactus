use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceAction {
    PlayHere,
    MoveThere,
    Toggle,
    Next,
    Previous,
}

impl DeviceAction {
    fn label(self, device: &str) -> String {
        match self {
            DeviceAction::PlayHere => fl!("device-play-here"),
            DeviceAction::MoveThere => fl!("device-move-there", device = device),
            DeviceAction::Toggle => fl!("menu-play-pause").replace('&', ""),
            DeviceAction::Next => fl!("menu-next").replace('&', ""),
            DeviceAction::Previous => fl!("menu-previous").replace('&', ""),
        }
    }
}

impl Ui {
    pub(super) fn show_devices(self: &Rc<Self>) {
        let title = fl!("menu-devices").replace('&', "");
        let (enabled, devices, active, own, problem) = {
            let player = self.player.borrow();
            (
                player.settings.ynison,
                player.devices().to_vec(),
                player.active_device().map(str::to_owned),
                player.device_id().to_owned(),
                player.ynison_problem().map(str::to_owned),
            )
        };
        if !enabled {
            return dialogs::warn(&self.frame, &fl!("devices-disabled"), &title);
        }
        if devices.is_empty() {
            let message =
                problem.map_or_else(|| fl!("devices-none"), |problem| fl!("devices-problem", problem = problem));
            return dialogs::warn(&self.frame, &message, &title);
        }
        let labels: Vec<String> = devices
            .iter()
            .map(|device| {
                let mut label = format!("{} ({})", device.title, device.kind);
                if active.as_deref() == Some(device.id.as_str()) {
                    label.push_str(&fl!("device-active"));
                }
                if device.id == own {
                    label.push_str(&fl!("device-this"));
                }
                label
            })
            .collect();
        let Some(index) = dialogs::choose(&self.frame, &fl!("devices-prompt"), &title, &labels) else {
            return;
        };
        let device = &devices[index];
        let is_active = active.as_deref() == Some(device.id.as_str());
        let actions: Vec<DeviceAction> = match (device.id == own, is_active) {
            (true, _) => Vec::new(),
            (false, true) => {
                vec![DeviceAction::PlayHere, DeviceAction::Toggle, DeviceAction::Next, DeviceAction::Previous]
            }
            (false, false) => vec![DeviceAction::MoveThere],
        };
        if actions.is_empty() {
            return;
        }
        let names: Vec<String> = actions.iter().map(|action| action.label(&device.title)).collect();
        let Some(choice) = dialogs::choose(&self.frame, &device.title, &title, &names) else {
            return;
        };
        let id = device.id.clone();
        self.act(|player| match actions[choice] {
            DeviceAction::PlayHere => player.play_here(),
            DeviceAction::MoveThere => player.move_to(&id),
            DeviceAction::Toggle => player.remote_toggle(),
            DeviceAction::Next => player.remote_skip(true),
            DeviceAction::Previous => player.remote_skip(false),
        });
    }
}
