use super::network::ProxyFields;
use super::*;
use tactus_api::AccountSettings;
use tactus_api::proxy::Proxy;
use tactus_core::player::Profile;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Overrides {
    pub alias: String,
    pub stream_quality: Option<Quality>,
    pub download_quality: Option<Quality>,
    pub proxy: Option<Proxy>,
}

impl Overrides {
    fn of(profile: &Profile) -> Self {
        let entry = &profile.entry;
        Self {
            alias: entry.alias.clone(),
            stream_quality: entry.stream_quality,
            download_quality: entry.download_quality,
            proxy: entry.proxy.clone(),
        }
    }
}

struct OverrideFields {
    alias: TextCtrl,
    stream_quality: Choice,
    download_quality: Choice,
    proxy_mode: Choice,
    proxy: ProxyFields,
}

const PROXY_MODES: usize = 3;

fn optional_quality(panel: &Panel, quality: Option<Quality>) -> Choice {
    let labels: Vec<String> =
        std::iter::once(fl!("account-same-as-app")).chain(Quality::ALL.into_iter().map(quality_label)).collect();
    let selected = quality.and_then(|quality| Quality::ALL.iter().position(|candidate| *candidate == quality));
    choice(panel, &labels, selected.map_or(0, |index| index + 1))
}

fn chosen_optional(control: &Choice) -> Option<Quality> {
    selection(control).checked_sub(1).and_then(|index| Quality::ALL.get(index).copied())
}

impl OverrideFields {
    fn new(panel: &Panel, sizer: &BoxSizer, initial: &Overrides) -> Self {
        let alias = field(sizer, panel, &fl!("account-alias"), true, || {
            TextCtrl::builder(panel).with_value(&initial.alias).build()
        });
        let stream_quality = field(sizer, panel, &fl!("settings-stream-quality"), false, || {
            optional_quality(panel, initial.stream_quality)
        });
        let download_quality = field(sizer, panel, &fl!("settings-download-quality"), false, || {
            optional_quality(panel, initial.download_quality)
        });
        let modes = [fl!("account-proxy-app"), fl!("account-proxy-none"), fl!("account-proxy-custom")];
        let mode = match &initial.proxy {
            None => 0,
            Some(proxy) if !proxy.enabled => 1,
            Some(_) => 2,
        };
        let proxy_mode = field(sizer, panel, &fl!("account-proxy"), false, || choice(panel, &modes, mode));
        let proxy = ProxyFields::new(panel, sizer, initial.proxy.as_ref().unwrap_or(&Proxy::default()));
        Self { alias, stream_quality, download_quality, proxy_mode, proxy }
    }

    fn read(&self) -> Overrides {
        Overrides {
            alias: self.alias.get_value().trim().to_owned(),
            stream_quality: chosen_optional(&self.stream_quality),
            download_quality: chosen_optional(&self.download_quality),
            proxy: match selection(&self.proxy_mode).min(PROXY_MODES - 1) {
                0 => None,
                1 => Some(Proxy::default()),
                _ => Some(self.proxy.read(true)),
            },
        }
    }
}

pub(super) struct Account {
    initial: AccountSettings,
    checks: Vec<(AccountOption, CheckBox)>,
    overrides: Option<(Overrides, OverrideFields)>,
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
    pub(super) fn new(notebook: &Notebook, profile: &Profile) -> Self {
        let initial = &profile.account_settings;
        let (panel, sizer) = page(notebook, &fl!("settings-account"));
        let overrides = profile.entry.is_saved().then(|| {
            add(&sizer, &label(&panel, &profile.label()));
            let current = Overrides::of(profile);
            let fields = OverrideFields::new(&panel, &sizer, &current);
            (current, fields)
        });
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
        Self { initial: initial.clone(), checks, overrides }
    }

    pub(super) fn overrides(&self) -> Option<Overrides> {
        let (initial, fields) = self.overrides.as_ref()?;
        Some(fields.read()).filter(|changed| changed != initial)
    }

    pub(super) fn changes(&self) -> Vec<(AccountOption, bool)> {
        self.checks
            .iter()
            .map(|(option, control)| (*option, control.is_checked()))
            .filter(|change| !self.initial.contains(change))
            .collect()
    }
}
