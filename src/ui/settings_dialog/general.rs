use super::*;

pub(super) struct General {
    pub(super) language: Choice,
    codes: Vec<String>,
    flags: Flags,
}

impl General {
    pub(super) fn new(notebook: &Notebook, old: &Settings) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-general"));
        let (codes, names): (Vec<String>, Vec<String>) = crate::i18n::languages().into_iter().unzip();
        let current = crate::i18n::current_language();
        let language = field(&sizer, &panel, &fl!("settings-language"), false, || {
            choice(&panel, &names, codes.iter().position(|code| *code == current).unwrap_or_default())
        });
        add(&sizer, &label(&panel, &fl!("settings-language-hint")));
        let mut current = old.clone();
        current.autostart = autostart::is_enabled();
        let flags = Flags::new(
            &panel,
            &sizer,
            &current,
            &[
                (fl!("settings-tray"), |s| &mut s.tray),
                (fl!("settings-minimize-to-tray"), |s| &mut s.minimize_to_tray),
                (fl!("settings-start-minimized"), |s| &mut s.start_minimized),
                (fl!("settings-autostart"), |s| &mut s.autostart),
                (fl!("settings-resume"), |s| &mut s.resume_session),
                (fl!("settings-autoplay"), |s| &mut s.autoplay),
                (fl!("settings-media-controls"), |s| &mut s.media_controls),
                (fl!("settings-notifications"), |s| &mut s.notifications),
                (fl!("settings-episode-notifications"), |s| &mut s.episode_notifications),
                (fl!("settings-check-updates"), |s| &mut s.check_updates),
            ],
        );
        panel.set_sizer(sizer, true);
        Self { language, codes, flags }
    }
}

impl Tab for General {
    fn apply(&self, settings: &mut Settings) {
        settings.language = self.codes.get(selection(&self.language)).cloned();
        self.flags.apply(settings);
    }
}
