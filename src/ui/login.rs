use super::widgets::*;
use crate::i18n::fl;
use tactus_core::player::LoginState;
use wxdragon::prelude::*;

pub struct LoginPanel {
    pub panel: Panel,
    pub get_code: Button,
    status: StaticText,
    code_label: StaticText,
    pub code: TextCtrl,
    pub copy_code: Button,
    pub open_page: Button,
}

impl LoginPanel {
    pub fn new(parent: &impl WxWidget) -> LoginPanel {
        let panel = Panel::builder(parent).build();
        let title = label(&panel, &fl!("login-title"));
        let description = label(&panel, &fl!("login-description"));
        description.wrap(560);
        let get_code = button(&panel, &fl!("login-get-code"));
        let status = label(&panel, "");
        let code_label = label(&panel, &fl!("login-code"));
        let code = TextCtrl::builder(&panel).with_style(TextCtrlStyle::ReadOnly).build();
        code.set_accessibility_label(&fl!("login-code"));
        let copy_code = button(&panel, &fl!("login-copy-code"));
        let open_page = button(&panel, &fl!("login-open-page"));
        let buttons = row();
        buttons.add(&copy_code, 0, SizerFlag::Right, 8);
        buttons.add(&open_page, 0, SizerFlag::Right, 8);
        let sizer = column();
        for widget in [&title, &description] {
            sizer.add(widget, 0, SizerFlag::All, 8);
        }
        sizer.add(&get_code, 0, SizerFlag::All, 8);
        sizer.add(&status, 0, SizerFlag::All, 8);
        sizer.add(&code_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
        sizer.add(&code, 0, SizerFlag::Expand | SizerFlag::All, 8);
        sizer.add_sizer(&buttons, 0, SizerFlag::All, 8);
        panel.set_sizer(sizer, true);
        LoginPanel { panel, get_code, status, code_label, code, copy_code, open_page }
    }

    pub fn update(&self, state: &LoginState) {
        let (status, code) = match state {
            LoginState::Idle => (String::new(), None),
            LoginState::Requesting => (fl!("login-requesting"), None),
            LoginState::Waiting { code, .. } => (fl!("login-instruction"), Some(code.as_str())),
            LoginState::Failed(failure) => (super::text::failure(failure), None),
        };
        self.status.set_label(&status);
        self.get_code.enable(*state != LoginState::Requesting);
        let waiting = code.is_some();
        self.code.change_value(code.unwrap_or_default());
        for widget in [&self.code_label as &dyn WxWidget, &self.code, &self.copy_code, &self.open_page] {
            widget.show(waiting);
        }
        self.panel.layout();
        if waiting {
            self.code.set_focus();
            self.code.set_selection(-1, -1);
        }
    }
}
