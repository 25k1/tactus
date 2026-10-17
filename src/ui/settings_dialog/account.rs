use super::*;
use tactus_api::AccountSettings;

pub(super) struct Account {
    initial: AccountSettings,
    checks: Vec<(AccountOption, CheckBox)>,
}

fn account_label(option: AccountOption) -> String {
    match option {
        AccountOption::ExplicitForbidden => fl!("account-explicit-forbidden"),
        AccountOption::ChildMode => fl!("account-child-mode"),
        AccountOption::AutoPlayRadio => fl!("account-autoplay-radio"),
        AccountOption::PublicCollection => fl!("account-public-collection"),
        AccountOption::AddToPlaylistTop => fl!("account-add-to-top"),
        AccountOption::SyncQueue => fl!("account-sync-queue"),
        AccountOption::ReduceAiContent => fl!("account-reduce-ai"),
    }
}

impl Account {
    pub(super) fn new(notebook: &Notebook, initial: &AccountSettings) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-account"));
        let checks: Vec<(AccountOption, CheckBox)> = initial
            .iter()
            .map(|(option, enabled)| (*option, check(&panel, &account_label(*option), *enabled)))
            .collect();
        if checks.is_empty() {
            add(&sizer, &label(&panel, &fl!("settings-account-unavailable")));
        }
        for (_, control) in &checks {
            add(&sizer, control);
        }
        panel.set_sizer(sizer, true);
        Self { initial: initial.clone(), checks }
    }

    pub(super) fn changes(&self) -> Vec<(AccountOption, bool)> {
        self.checks
            .iter()
            .map(|(option, control)| (*option, control.is_checked()))
            .filter(|change| !self.initial.contains(change))
            .collect()
    }
}
