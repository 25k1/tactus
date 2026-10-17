use super::*;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use tactus_core::settings::DEFAULT_REMOTE_PORT;

pub(super) struct Remote {
    enabled: CheckBox,
    port: SpinCtrl,
    require_token: CheckBox,
    token: TextCtrl,
}

fn new_token() -> String {
    (0..2).map(|_| format!("{:016x}", RandomState::new().build_hasher().finish())).collect()
}

impl Remote {
    pub(super) fn new(notebook: &Notebook, old: &Settings) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-remote"));
        let enabled = check(&panel, &fl!("settings-remote-enabled"), old.remote_enabled);
        add(&sizer, &enabled);
        let port = field(&sizer, &panel, &fl!("settings-remote-port"), false, || {
            SpinCtrl::builder(&panel)
                .with_min_value(1024)
                .with_max_value(65535)
                .with_initial_value(i32::from(old.remote_port))
                .build()
        });
        let require_token = check(&panel, &fl!("settings-remote-token-required"), !old.remote_token.is_empty());
        add(&sizer, &require_token);
        let token = field(&sizer, &panel, &fl!("settings-remote-token"), true, || {
            TextCtrl::builder(&panel).with_style(TextCtrlStyle::ReadOnly).with_value(&old.remote_token).build()
        });
        let line = row();
        let [generate, copy] =
            [fl!("settings-remote-generate"), fl!("settings-remote-copy")].map(|text| button(&panel, &text));
        line.add(&generate, 0, centered(), 6);
        line.add(&copy, 0, centered(), 6);
        sizer.add_sizer(&line, 0, SizerFlag::All, 6);
        add(&sizer, &label(&panel, &fl!("settings-remote-hint")));
        panel.set_sizer(sizer, true);
        generate.on_click(move |_| {
            token.change_value(&new_token());
            require_token.set_value(true);
        });
        copy.on_click(move |_| {
            let value = token.get_value();
            if !value.is_empty() {
                Clipboard::get().set_text(&value);
            }
        });
        require_token.on_toggled(move |event| {
            if event.is_checked() && token.get_value().is_empty() {
                token.change_value(&new_token());
            }
        });
        Self { enabled, port, require_token, token }
    }
}

impl Tab for Remote {
    fn apply(&self, settings: &mut Settings) {
        settings.remote_enabled = self.enabled.is_checked();
        settings.remote_port = u16::try_from(self.port.value()).unwrap_or(DEFAULT_REMOTE_PORT);
        settings.remote_token = if self.require_token.is_checked() { self.token.get_value() } else { String::new() };
    }
}
