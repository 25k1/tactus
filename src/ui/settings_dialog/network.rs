use super::*;
use tactus_api::proxy::{Proxy, ProxyKind};

pub(super) struct Network {
    enabled: CheckBox,
    kind: Choice,
    host: TextCtrl,
    port: SpinCtrl,
    login: TextCtrl,
    password: TextCtrl,
}

impl Network {
    pub(super) fn new(notebook: &Notebook, old: &Settings) -> Self {
        let proxy = &old.proxy;
        let (panel, sizer) = page(notebook, &fl!("settings-network"));
        let enabled = check(&panel, &fl!("settings-proxy-enabled"), proxy.enabled);
        add(&sizer, &enabled);
        let kinds: Vec<String> = ProxyKind::ALL.iter().map(|kind| kind.name().to_owned()).collect();
        let selected = ProxyKind::ALL.iter().position(|kind| *kind == proxy.kind).unwrap_or_default();
        let kind = field(&sizer, &panel, &fl!("settings-proxy-kind"), false, || choice(&panel, &kinds, selected));
        let host = field(&sizer, &panel, &fl!("settings-proxy-host"), true, || {
            TextCtrl::builder(&panel).with_value(&proxy.host).build()
        });
        let port = field(&sizer, &panel, &fl!("settings-proxy-port"), false, || {
            SpinCtrl::builder(&panel)
                .with_min_value(1)
                .with_max_value(65535)
                .with_initial_value(i32::from(proxy.port))
                .build()
        });
        let login = field(&sizer, &panel, &fl!("settings-proxy-login"), true, || {
            TextCtrl::builder(&panel).with_value(&proxy.login).build()
        });
        let password = field(&sizer, &panel, &fl!("settings-proxy-password"), true, || {
            TextCtrl::builder(&panel).with_style(TextCtrlStyle::Password).with_value(&proxy.password).build()
        });
        add(&sizer, &label(&panel, &fl!("settings-proxy-hint")));
        panel.set_sizer(sizer, true);
        Self { enabled, kind, host, port, login, password }
    }
}

impl Tab for Network {
    fn apply(&self, settings: &mut Settings) {
        settings.proxy = Proxy {
            enabled: self.enabled.is_checked(),
            kind: ProxyKind::ALL[selection(&self.kind).min(ProxyKind::ALL.len() - 1)],
            host: self.host.get_value().trim().to_owned(),
            port: u16::try_from(self.port.value()).unwrap_or(Proxy::default().port),
            login: self.login.get_value().trim().to_owned(),
            password: self.password.get_value(),
        };
    }
}
