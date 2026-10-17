use super::*;
use tactus_core::audio::equalizer::{FREQUENCIES, MAX_GAIN_DB};
use tactus_core::audio::{BANDS, Preset};

pub(super) struct Equalizer {
    enabled: CheckBox,
    bands: Rc<Vec<Slider>>,
}

fn preset_label(preset: Preset) -> String {
    match preset {
        Preset::Flat => fl!("preset-flat"),
        Preset::Bass => fl!("preset-bass"),
        Preset::Treble => fl!("preset-treble"),
        Preset::Vocal => fl!("preset-vocal"),
        Preset::Speech => fl!("preset-speech"),
        Preset::Rock => fl!("preset-rock"),
        Preset::Pop => fl!("preset-pop"),
        Preset::Jazz => fl!("preset-jazz"),
        Preset::Classical => fl!("preset-classical"),
        Preset::Electronic => fl!("preset-electronic"),
    }
}

fn frequency_label(frequency: f32) -> String {
    if frequency >= 1000.0 {
        fl!("band-khz", frequency = format!("{}", frequency / 1000.0))
    } else {
        fl!("band-hz", frequency = format!("{frequency}"))
    }
}

fn preset_index(gains: &Gains) -> usize {
    Preset::matching(gains)
        .and_then(|preset| Preset::ALL.iter().position(|candidate| *candidate == preset))
        .unwrap_or(Preset::ALL.len())
}

fn gains(bands: &[Slider]) -> Gains {
    std::array::from_fn(|band| bands[band].get_value() as f32)
}

impl Equalizer {
    pub(super) fn new(notebook: &Notebook, old: &Settings, preview: Rc<dyn Fn(&Gains)>) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-equalizer"));
        let enabled = check(&panel, &fl!("settings-equalizer-enabled"), old.equalizer);
        add(&sizer, &enabled);
        let presets: Vec<String> =
            Preset::ALL.into_iter().map(preset_label).chain(std::iter::once(fl!("preset-custom"))).collect();
        let preset = field(&sizer, &panel, &fl!("settings-equalizer-preset"), false, || {
            choice(&panel, &presets, preset_index(&old.equalizer_gains))
        });
        let bands: Rc<Vec<Slider>> = Rc::new(
            FREQUENCIES
                .iter()
                .zip(old.equalizer_gains)
                .map(|(frequency, gain)| {
                    field(&sizer, &panel, &frequency_label(*frequency), true, || {
                        Slider::builder(&panel)
                            .with_min_value(-(MAX_GAIN_DB as i32))
                            .with_max_value(MAX_GAIN_DB as i32)
                            .with_value(gain.round() as i32)
                            .build()
                    })
                })
                .collect(),
        );
        add(&sizer, &label(&panel, &fl!("settings-equalizer-hint")));
        panel.set_sizer(sizer, true);
        let refresh = {
            let bands = bands.clone();
            Rc::new(move || preview(&if enabled.is_checked() { gains(&bands) } else { [0.0; BANDS] }))
        };
        {
            let (refresh, bands) = (refresh.clone(), bands.clone());
            preset.on_selection_changed(move |_| {
                if let Some(chosen) = Preset::ALL.get(selection(&preset)) {
                    for (slider, gain) in bands.iter().zip(chosen.gains()) {
                        slider.set_value(gain as i32);
                    }
                    refresh();
                }
            });
        }
        for slider in bands.iter() {
            let (refresh, bands) = (refresh.clone(), bands.clone());
            slider.on_slider(move |_| {
                preset.set_selection(preset_index(&gains(&bands)) as u32);
                refresh();
            });
        }
        enabled.on_toggled(move |_| refresh());
        Self { enabled, bands }
    }
}

impl Tab for Equalizer {
    fn apply(&self, settings: &mut Settings) {
        settings.equalizer = self.enabled.is_checked();
        settings.equalizer_gains = gains(&self.bands);
    }
}
