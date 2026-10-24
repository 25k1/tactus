use super::view_model;
use super::widgets::*;
use crate::i18n::fl;
use tactus_core::player::Player;
use tactus_core::player::queue::Repeat;
use wxdragon::prelude::*;

pub trait PlayerView {
    fn update_track(&self, player: &Player);
    fn update_playback(&self, player: &Player);
    fn update_volume(&self, player: &Player);
    fn update_modes(&self, player: &Player);
}

pub struct PlayerBar {
    pub panel: Panel,
    pub toggle: Button,
    controls: Panel,
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

pub fn play_label(player: &Player) -> String {
    if player.playing { fl!("player-pause") } else { fl!("player-play") }
}

pub fn volume_slider(parent: &impl WxWidget, volume: u8) -> Slider {
    Slider::builder(parent)
        .with_min_value(0)
        .with_max_value(100)
        .with_value(i32::from(volume))
        .with_size(Size::new(140, -1))
        .build()
}

impl PlayerBar {
    pub fn new(parent: &impl WxWidget, volume: u8, compact: bool) -> PlayerBar {
        let panel = Panel::builder(parent).build();
        let toggle = button(&panel, "");
        let now_playing = label(&panel, &fl!("now-playing-nothing"));
        let controls = Panel::builder(&panel).build();
        let line = row();
        let [previous, play, next] = [fl!("player-previous"), fl!("player-play"), fl!("player-next")].map(|text| {
            let control = button(&controls, &text);
            line.add(&control, 0, centered(), 6);
            control
        });
        line.add(&label(&controls, &fl!("player-position")), 0, centered(), 4);
        let position = Slider::builder(&controls).with_min_value(0).with_max_value(1).build();
        line.add(&position, 1, centered(), 6);
        let position_label = label(&controls, "0:00 / 0:00");
        line.add(&position_label, 0, centered(), 16);
        line.add(&label(&controls, &fl!("player-volume")), 0, centered(), 4);
        let volume = volume_slider(&controls, volume);
        line.add(&volume, 0, centered(), 16);
        line.add(&label(&controls, &fl!("player-repeat")), 0, centered(), 4);
        let repeats: Vec<String> = Repeat::ALL.into_iter().map(repeat_label).collect();
        let repeat = choice(&controls, &repeats, 0);
        line.add(&repeat, 0, centered(), 12);
        let shuffle = check(&controls, &fl!("player-shuffle"), false);
        line.add(&shuffle, 0, SizerFlag::AlignCenterVertical, 0);
        controls.set_sizer(line, true);
        let top = row();
        top.add(&toggle, 0, centered(), 8);
        top.add(&now_playing, 1, SizerFlag::AlignCenterVertical, 0);
        let sizer = column();
        sizer.add_sizer(&top, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        sizer.add(&controls, 0, SizerFlag::Expand | SizerFlag::All, 6);
        panel.set_sizer(sizer, true);
        let bar = PlayerBar {
            panel,
            toggle,
            controls,
            now_playing,
            previous,
            play,
            next,
            position,
            position_label,
            volume,
            repeat,
            shuffle,
        };
        bar.set_compact(compact);
        bar
    }

    pub fn set_compact(&self, compact: bool) {
        self.controls.show(!compact);
        self.toggle.set_label(&if compact { fl!("player-show-controls") } else { fl!("player-hide-controls") });
        self.panel.layout();
        if let Some(parent) = self.panel.get_parent() {
            parent.layout();
        }
    }

    pub fn has_focus(&self) -> bool {
        self.toggle.has_focus()
            || [
                &self.previous as &dyn WxWidget,
                &self.play,
                &self.next,
                &self.position,
                &self.volume,
                &self.repeat,
                &self.shuffle,
            ]
            .iter()
            .any(|control| control.has_focus())
    }

    pub fn focus(&self) {
        if self.controls.is_shown() { self.play.set_focus() } else { self.toggle.set_focus() }
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
}

impl PlayerView for PlayerBar {
    fn update_track(&self, player: &Player) {
        self.now_playing.set_label(&view_model::now_playing(player).unwrap_or_else(|| fl!("now-playing-nothing")));
        self.update_position(player);
    }

    fn update_playback(&self, player: &Player) {
        self.play.set_label(&play_label(player));
    }

    fn update_volume(&self, player: &Player) {
        self.volume.set_value(i32::from(player.settings.volume));
    }

    fn update_modes(&self, player: &Player) {
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
