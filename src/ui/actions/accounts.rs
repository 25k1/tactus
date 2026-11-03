use super::*;
use crate::ui::command::AccountMenu;
use tactus_api::ProviderKind;

impl Ui {
    pub(crate) fn account_menu(&self) -> AccountMenu {
        let player = self.player.borrow();
        let accounts = player.accounts();
        AccountMenu {
            labels: accounts.iter().map(|profile| profile.label()).collect(),
            active: accounts.iter().position(|profile| profile.id() == player.profile.id()),
            capabilities: player.capabilities(),
        }
    }

    pub(super) fn switch_to(self: &Rc<Self>, index: usize) {
        let id = self.player.borrow().accounts().get(index).map(|profile| profile.id().to_owned());
        let Some(id) = id else {
            return;
        };
        self.act(|player| player.switch_account(&id));
        if let Some(view) = self.current_view() {
            self.act(|player| player.load(&view, false));
        }
        self.render(None, None);
    }

    pub(super) fn choose_account(self: &Rc<Self>) {
        let menu = self.account_menu();
        if menu.labels.len() < 2 {
            return self.speech.say(&fl!("accounts-only-one"), true);
        }
        let prompt = fl!("switch-account-prompt");
        if let Some(index) = dialogs::choose(&self.frame, &prompt, &fl!("menu-switch-account"), &menu.labels) {
            self.switch_to(index);
        }
    }

    pub(super) fn add_account(self: &Rc<Self>) {
        self.act(|player| player.begin_login(ProviderKind::Yandex));
    }

    pub(super) fn rename_account(self: &Rc<Self>) {
        let current = self.player.borrow().profile.entry.alias.clone();
        let prompt = fl!("rename-account-prompt");
        if let Some(alias) = dialogs::ask_text(&self.frame, &prompt, &fl!("menu-rename-account"), &current) {
            self.act(|player| player.rename_account(&alias));
        }
    }
}
