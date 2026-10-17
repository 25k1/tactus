use super::*;
use tactus_core::download::TEMPLATES;

pub(super) struct Downloads {
    folder: DirPickerCtrl,
    quality: Choice,
    template: ComboBox,
    cover: CheckBox,
}

impl Downloads {
    pub(super) fn new(notebook: &Notebook, old: &Settings) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-downloads"));
        let folder = field(&sizer, &panel, &fl!("settings-download-folder"), true, || {
            DirPickerCtrl::builder(&panel)
                .with_message(&fl!("settings-download-folder"))
                .with_path(&old.download_folder().to_string_lossy())
                .build()
        });
        let quality = field(&sizer, &panel, &fl!("settings-download-quality"), false, || {
            quality_choice(&panel, old.download_quality)
        });
        let template = field(&sizer, &panel, &fl!("settings-download-template"), true, || {
            editable(&panel, &TEMPLATES, &old.download_template)
        });
        add(&sizer, &label(&panel, &fl!("settings-download-template-hint")));
        let cover = check(&panel, &fl!("settings-embed-cover"), old.embed_cover);
        add(&sizer, &cover);
        panel.set_sizer(sizer, true);
        Self { folder, quality, template, cover }
    }
}

impl Tab for Downloads {
    fn apply(&self, settings: &mut Settings) {
        settings.download_dir = tactus_core::settings::portable_path(&self.folder.get_path());
        settings.download_quality = chosen_quality(&self.quality);
        settings.download_template = self.template.get_value().trim().to_owned();
        settings.embed_cover = self.cover.is_checked();
    }
}
