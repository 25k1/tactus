use super::dialogs::caption;
use super::widgets::*;
use crate::i18n::fl;
use tactus_api::{WaveOption, WaveSetting, WaveSettings};
use tactus_core::player::wave;
use wxdragon::prelude::*;

fn built_in() -> Vec<WaveSetting> {
    fn setting<const N: usize>(key: &str, name: String, labels: [String; N], seeds: [&str; N]) -> WaveSetting {
        let any = WaveOption { name: fl!("wave-any"), seed: String::new(), any: true };
        let options =
            labels.into_iter().zip(seeds).map(|(name, seed)| WaveOption { name, seed: seed.to_owned(), any: false });
        WaveSetting { key: key.to_owned(), name, options: options.chain(std::iter::once(any)).collect() }
    }
    vec![
        setting(
            "diversity",
            fl!("wave-character"),
            [fl!("wave-favorite"), fl!("wave-discover"), fl!("wave-popular")],
            wave::CHARACTERS,
        ),
        setting(
            "moodEnergy",
            fl!("wave-mood"),
            [fl!("wave-active"), fl!("wave-fun"), fl!("wave-calm"), fl!("wave-sad")],
            wave::MOODS,
        ),
        setting(
            "language",
            fl!("wave-language"),
            [fl!("wave-russian"), fl!("wave-foreign"), fl!("wave-instrumental")],
            wave::LANGUAGES,
        ),
    ]
}

fn chosen(setting: &WaveSetting, current: &[String]) -> usize {
    let saved = setting.options.iter().position(|option| !option.any && current.contains(&option.seed));
    saved.or_else(|| setting.options.iter().position(|option| option.any)).unwrap_or_default()
}

pub fn edit(parent: &Frame, options: Option<&WaveSettings>, current: &[String]) -> Option<Vec<String>> {
    let settings = options.map(|options| options.settings.clone()).filter(|settings| !settings.is_empty());
    let settings = settings.unwrap_or_else(built_in);
    let title = caption(&fl!("menu-wave-settings"));
    let dialog = Dialog::builder(parent, title.trim_end_matches('…')).build();
    let sizer = column();
    let choices: Vec<Choice> = settings
        .iter()
        .map(|setting| {
            let labels: Vec<String> = setting.options.iter().map(|option| option.name.clone()).collect();
            let name = if setting.name.is_empty() { setting.key.clone() } else { setting.name.clone() };
            field(&sizer, &dialog, &name, true, || choice(&dialog, &labels, chosen(setting, current)))
        })
        .collect();
    sizer.add(&label(&dialog, &fl!("wave-settings-hint")), 0, SizerFlag::Expand | SizerFlag::All, 8);
    let buttons = row();
    let ok = Button::builder(&dialog).with_id(ID_OK).with_label(&fl!("dialog-ok")).build();
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&fl!("dialog-cancel")).build();
    buttons.add(&ok, 0, SizerFlag::All, 6);
    buttons.add(&cancel, 0, SizerFlag::All, 6);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 6);
    dialog.set_sizer_and_fit(sizer, true);
    dialog.set_escape_id(ID_CANCEL);
    ok.set_default();
    if let Some(first) = choices.first() {
        first.set_focus();
    }
    let accepted = dialog.show_modal() == ID_OK;
    let seeds = choices
        .iter()
        .zip(&settings)
        .filter_map(|(control, setting)| setting.options.get(selection(control)))
        .filter(|option| !option.any && !option.seed.is_empty())
        .map(|option| option.seed.clone())
        .collect();
    dialog.destroy();
    accepted.then_some(seeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting() -> WaveSetting {
        let option = |seed: &str, any: bool| WaveOption { name: seed.into(), seed: seed.into(), any };
        WaveSetting {
            key: "language".into(),
            name: "Language".into(),
            options: vec![option("settingLanguage:russian", false), option("settingLanguage:any", true)],
        }
    }

    #[test]
    fn selects_the_saved_option_or_any() {
        assert_eq!(chosen(&setting(), &["settingLanguage:russian".into()]), 0);
        assert_eq!(chosen(&setting(), &[]), 1);
    }
}
