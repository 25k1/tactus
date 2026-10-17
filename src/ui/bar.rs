use super::view_model;
use super::widgets::*;
use crate::i18n::fl;
use tactus_core::player::Player;
use tactus_core::player::queue::Repeat;
use wxdragon::prelude::*;

pub struct PlayerBar {
    pub panel: Panel,
    pub now_playing: StaticText,
    pub previous: Button,
    pub play: Button,
    pub next: Button,
    pub position: Slider,
    position_label: StaticText,
    pub volume: Slider,
    pub repeat: Choice,
    pub shuffle: CheckBox,
}

fn repeat_label(repeat: Repeat) -> String {
    match repeat {
        Repeat::Off => fl!("repeat-off"),
        Repeat::One => fl!("repeat-one"),
        Repeat::All => fl!("repeat-all"),
    }
}

impl PlayerBar {
    pub fn new(parent: &impl WxWidget, volume: u8) -> PlayerBar {
        let panel = Panel::builder(parent).build();
        let now_playing = label(&panel, &fl!("now-playing-nothing"));
        let controls = row();
        let [previous, play, next] = [fl!("player-previous"), fl!("player-play"), fl!("player-next")].map(|text| {
            let control = button(&panel, &text);
            controls.add(&control, 0, centered(), 6);
            control
        });
        controls.add(&label(&panel, &fl!("player-position")), 0, centered(), 4);
        let position = Slider::builder(&panel).with_min_value(0).with_max_value(1).build();
        controls.add(&position, 1, centered(), 6);
        let position_label = label(&panel, "0:00 / 0:00");
        controls.add(&position_label, 0, centered(), 16);
        controls.add(&label(&panel, &fl!("player-volume")), 0, centered(), 4);
        let volume = Slider::builder(&panel)
            .with_min_value(0)
            .with_max_value(100)
            .with_value(i32::from(volume))
            .with_size(Size::new(140, -1))
            .build();
        controls.add(&volume, 0, centered(), 16);
        controls.add(&label(&panel, &fl!("player-repeat")), 0, centered(), 4);
        let repeats: Vec<String> = Repeat::ALL.into_iter().map(repeat_label).collect();
        let repeat = choice(&panel, &repeats, 0);
        controls.add(&repeat, 0, centered(), 12);
        let shuffle = check(&panel, &fl!("player-shuffle"), false);
        controls.add(&shuffle, 0, SizerFlag::AlignCenterVertical, 0);
        let sizer = column();
        sizer.add(&now_playing, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        sizer.add_sizer(&controls, 0, SizerFlag::Expand | SizerFlag::All, 6);
        panel.set_sizer(sizer, true);
        PlayerBar { panel, now_playing, previous, play, next, position, position_label, volume, repeat, shuffle }
    }

    pub fn update_track(&self, player: &Player) {
        let text = view_model::now_playing(player).unwrap_or_else(|| fl!("now-playing-nothing"));
        self.now_playing.set_label(&text);
        self.update_position(player);
    }

    pub fn update_playback(&self, player: &Player) {
        self.play.set_label(&if player.playing { fl!("player-pause") } else { fl!("player-play") });
    }

    pub fn update_position(&self, player: &Player) {
        self.position_label.set_label(&view_model::position(player));
        if self.position.has_focus() {
            return;
        }
        let duration = player.track.as_ref().map_or(0, |track| track.duration_ms / 1000).max(1) as i32;
        if self.position.max() != duration {
            self.position.set_range(0, duration);
        }
        self.position.set_value(((player.position_ms / 1000) as i32).min(duration));
    }

    pub fn update_volume(&self, player: &Player) {
        self.volume.set_value(i32::from(player.settings.volume));
    }

    pub fn update_modes(&self, player: &Player) {
        let index = Repeat::ALL.iter().position(|repeat| *repeat == player.settings.repeat).unwrap_or_default();
        self.repeat.set_selection(index as u32);
        self.shuffle.set_value(player.settings.shuffle);
    }
}

pub fn repeat_at(index: usize) -> Repeat {
    Repeat::ALL.get(index).copied().unwrap_or_default()
}

pub fn modes_text(player: &Player) -> String {
    let shuffle = if player.settings.shuffle { fl!("announce-shuffle-on") } else { fl!("announce-shuffle-off") };
    format!("{}, {}", repeat_label(player.settings.repeat), shuffle)
}

pub fn speed_text(player: &Player) -> String {
    fl!(
        "announce-speed",
        speed = format!("{:.2}", player.speed()).trim_end_matches('0').trim_end_matches('.').to_owned()
    )
}
