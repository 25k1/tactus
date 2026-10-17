use super::*;
use tactus_core::settings::MAX_CROSSFADE;

const SPEEDS: [f32; 9] = [0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

pub(super) struct Playback {
    quality: Choice,
    device: Choice,
    devices: Vec<String>,
    speed: Choice,
    spoken_speed: Choice,
    crossfade: SpinCtrl,
    device_name: TextCtrl,
    flags: Flags,
}

fn nearest_speed(speed: f32) -> usize {
    SPEEDS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - speed).abs().total_cmp(&(*b - speed).abs()))
        .map_or(3, |(index, _)| index)
}

impl Playback {
    pub(super) fn new(notebook: &Notebook, old: &Settings) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-playback"));
        let quality = field(&sizer, &panel, &fl!("settings-stream-quality"), false, || {
            quality_choice(&panel, old.stream_quality)
        });
        let devices = tactus_core::audio::devices();
        let names: Vec<String> =
            std::iter::once(fl!("settings-default-device")).chain(devices.iter().cloned()).collect();
        let current = devices.iter().position(|name| *name == old.output_device).map_or(0, |index| index + 1);
        let device = field(&sizer, &panel, &fl!("settings-output-device"), true, || choice(&panel, &names, current));
        let labels: Vec<String> = SPEEDS.iter().map(|speed| format!("{speed}×")).collect();
        let speed =
            field(&sizer, &panel, &fl!("settings-speed"), false, || choice(&panel, &labels, nearest_speed(old.speed)));
        let spoken_speed = field(&sizer, &panel, &fl!("settings-spoken-speed"), false, || {
            choice(&panel, &labels, nearest_speed(old.spoken_speed))
        });
        let crossfade = field(&sizer, &panel, &fl!("settings-crossfade"), false, || {
            SpinCtrl::builder(&panel)
                .with_min_value(0)
                .with_max_value(i32::from(MAX_CROSSFADE))
                .with_initial_value(i32::from(old.crossfade))
                .build()
        });
        let flags = Flags::new(
            &panel,
            &sizer,
            old,
            &[
                (fl!("settings-normalize"), |s| &mut s.normalize),
                (fl!("settings-continue-wave"), |s| &mut s.continue_with_wave),
                (fl!("settings-sync-queue"), |s| &mut s.sync_queue),
                (fl!("settings-ynison"), |s| &mut s.ynison),
            ],
        );
        let device_name = field(&sizer, &panel, &fl!("settings-device-name"), true, || {
            TextCtrl::builder(&panel).with_value(&old.device_name).build()
        });
        add(
            &sizer,
            &label(&panel, &fl!("settings-ynison-hint", name = tactus_core::player::ynison::default_device_name())),
        );
        panel.set_sizer(sizer, true);
        Self { quality, device, devices, speed, spoken_speed, crossfade, device_name, flags }
    }
}

impl Tab for Playback {
    fn apply(&self, settings: &mut Settings) {
        settings.stream_quality = chosen_quality(&self.quality);
        settings.output_device = selection(&self.device)
            .checked_sub(1)
            .and_then(|index| self.devices.get(index).cloned())
            .unwrap_or_default();
        settings.speed = SPEEDS.get(selection(&self.speed)).copied().unwrap_or(1.0);
        settings.spoken_speed = SPEEDS.get(selection(&self.spoken_speed)).copied().unwrap_or(1.0);
        settings.crossfade = u8::try_from(self.crossfade.value()).unwrap_or_default().min(MAX_CROSSFADE);
        settings.device_name = self.device_name.get_value().trim().to_owned();
        self.flags.apply(settings);
    }
}
