use super::*;
use std::collections::BTreeMap;
use tactus_core::voice::{self, ModelSize, MusicWhileListening, VoiceSettings};

const LANGUAGES: [&str; 3] = ["", "ru", "en"];
const SIZES: [ModelSize; 2] = [ModelSize::Small, ModelSize::Large];
const MUSIC: [MusicWhileListening; 3] =
    [MusicWhileListening::Duck, MusicWhileListening::Pause, MusicWhileListening::Keep];

type Sizes = Rc<RefCell<BTreeMap<String, ModelSize>>>;

pub(super) struct Voice {
    language: Choice,
    size: Choice,
    sizes: Sizes,
    microphone: Choice,
    microphones: Vec<String>,
    music: Choice,
    timings: [SpinCtrl; 3],
    flags: Flags,
}

fn language_label(code: &str) -> String {
    match code {
        "ru" => fl!("voice-language-ru"),
        "en" => fl!("voice-language-en"),
        _ => fl!("voice-language-auto"),
    }
}

fn size_label(language: &str, size: ModelSize) -> String {
    let megabytes = voice::model(language, size).size / 1_000_000;
    match size {
        ModelSize::Small => fl!("voice-model-small", megabytes = megabytes),
        ModelSize::Large => fl!("voice-model-large", megabytes = megabytes),
    }
}

fn picked(language: &Choice) -> &'static str {
    LANGUAGES.get(selection(language)).copied().unwrap_or_default()
}

fn spoken(language: &Choice) -> &'static str {
    let probe = VoiceSettings { language: picked(language).to_owned(), ..VoiceSettings::default() };
    probe.language_for(&crate::i18n::current_language())
}

fn size_of(sizes: &Sizes, code: &str) -> ModelSize {
    sizes.borrow().get(code).copied().unwrap_or_default()
}

fn download_label(installing: bool) -> String {
    if installing { fl!("voice-cancel-download") } else { fl!("voice-download") }
}

fn status(language: &str, size: ModelSize) -> String {
    if voice::ready(language, size) { fl!("voice-model-installed") } else { fl!("voice-model-missing") }
}

fn spin(panel: &Panel, sizer: &BoxSizer, text: &str, value: u32, range: (u32, u32)) -> SpinCtrl {
    field(sizer, panel, text, false, || {
        SpinCtrl::builder(panel)
            .with_min_value(range.0 as i32)
            .with_max_value(range.1 as i32)
            .with_initial_value(value as i32)
            .build()
    })
}

impl Voice {
    pub(super) fn new(notebook: &Notebook, old: &Settings, ui: &Rc<Ui>) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-voice"));
        let settings = &old.voice;
        let flags = Flags::new(&panel, &sizer, old, &[(fl!("voice-enabled"), |s| &mut s.voice.enabled)]);
        let labels: Vec<String> = LANGUAGES.iter().map(|code| language_label(code)).collect();
        let current = LANGUAGES.iter().position(|code| *code == settings.language).unwrap_or_default();
        let language = field(&sizer, &panel, &fl!("voice-language"), false, || choice(&panel, &labels, current));
        let sizes: Sizes = Rc::new(RefCell::new(settings.models.clone()));
        let (caption, size) = labeled_field(&sizer, &panel, "", true, || choice(&panel, &[], 0));
        let state = label(&panel, "");
        add(&sizer, &state);
        let refresh = {
            let sizes = sizes.clone();
            Rc::new(move || {
                let code = spoken(&language);
                let chosen = size_of(&sizes, code);
                caption.set_label(&fl!("voice-model", language = language_label(code)));
                let labels: Vec<String> = SIZES.iter().map(|size| size_label(code, *size)).collect();
                set_choices(&size, &labels, SIZES.iter().position(|size| *size == chosen).unwrap_or_default());
                state.set_label(&status(code, chosen));
            })
        };
        refresh();
        let line = row();
        let installing = ui.player.borrow().installing_voice();
        let [download, remove, phrases] =
            [download_label(installing), fl!("voice-remove"), fl!("voice-phrases")].map(|text| button(&panel, &text));
        for control in [&download, &remove, &phrases] {
            line.add(control, 0, centered(), 6);
        }
        sizer.add_sizer(&line, 0, SizerFlag::All, 6);
        let microphones = tactus_core::audio::input_devices();
        let names: Vec<String> =
            std::iter::once(fl!("settings-default-device")).chain(microphones.iter().cloned()).collect();
        let current = microphones.iter().position(|name| *name == settings.microphone).map_or(0, |index| index + 1);
        let microphone = field(&sizer, &panel, &fl!("voice-microphone"), true, || choice(&panel, &names, current));
        let music_labels = [fl!("voice-music-duck"), fl!("voice-music-pause"), fl!("voice-music-keep")];
        let music_index = MUSIC.iter().position(|music| *music == settings.music).unwrap_or_default();
        let music = field(&sizer, &panel, &fl!("voice-music"), false, || choice(&panel, &music_labels, music_index));
        let timings = [
            spin(&panel, &sizer, &fl!("voice-silence"), settings.silence_ms, (300, 3_000)),
            spin(&panel, &sizer, &fl!("voice-wait"), settings.wait_ms / 1000, (1, 30)),
            spin(&panel, &sizer, &fl!("voice-limit"), settings.limit_ms / 1000, (2, 60)),
        ];
        let more = Flags::new(
            &panel,
            &sizer,
            old,
            &[(fl!("voice-wake-word"), |s| &mut s.voice.wake_word), (fl!("voice-log"), |s| &mut s.voice.log)],
        );
        add(&sizer, &label(&panel, &fl!("voice-hint")));
        panel.set_sizer(sizer, true);
        let chosen = move || SIZES.get(selection(&size)).copied().unwrap_or_default();
        {
            let refresh = refresh.clone();
            language.on_selection_changed(move |_| refresh());
        }
        {
            let sizes = sizes.clone();
            size.on_selection_changed(move |_| {
                let code = spoken(&language);
                sizes.borrow_mut().insert(code.to_owned(), chosen());
                state.set_label(&status(code, chosen()));
            });
        }
        {
            let ui = ui.clone();
            download.on_click(move |_| {
                let (code, size) = (spoken(&language), chosen());
                let installing = ui.player.borrow().installing_voice();
                if installing {
                    ui.act(Player::cancel_voice_install);
                    state.set_label(&status(code, size));
                } else {
                    ui.act(|player| player.install_voice(code, size));
                    state.set_label(&fl!("voice-model-downloading"));
                }
                download.set_label(&download_label(!installing));
            });
        }
        {
            let ui = ui.clone();
            remove.on_click(move |_| {
                let (code, size) = (spoken(&language), chosen());
                ui.act(|player| player.remove_voice_model(code, size));
                state.set_label(&status(code, size));
            });
        }
        phrases.on_click(move |_| {
            let file = voice::phrases_file(spoken(&language));
            if !file.exists() {
                let _ = std::fs::create_dir_all(voice::folder());
                let _ = std::fs::write(&file, fl!("voice-phrases-header"));
            }
            crate::platform::open(&file);
        });
        let Flags(mut items) = flags;
        items.extend(more.0);
        Self { language, size, sizes, microphone, microphones, music, timings, flags: Flags(items) }
    }
}

impl Tab for Voice {
    fn apply(&self, settings: &mut Settings) {
        self.sizes
            .borrow_mut()
            .insert(spoken(&self.language).to_owned(), SIZES.get(selection(&self.size)).copied().unwrap_or_default());
        let voice = &mut settings.voice;
        voice.language = picked(&self.language).to_owned();
        voice.models = self.sizes.borrow().clone();
        voice.microphone = selection(&self.microphone)
            .checked_sub(1)
            .and_then(|index| self.microphones.get(index).cloned())
            .unwrap_or_default();
        voice.music = MUSIC.get(selection(&self.music)).copied().unwrap_or_default();
        let [silence, wait, limit] =
            self.timings.each_ref().map(|control| u32::try_from(control.value()).unwrap_or_default());
        (voice.silence_ms, voice.wait_ms, voice.limit_ms) = (silence, wait * 1000, limit * 1000);
        voice.clamp();
        self.flags.apply(settings);
    }
}
