use super::bar::{PlayerView, play_label, speed_text, volume_slider};
use super::view_model;
use super::widgets::*;
use crate::i18n::fl;
use tactus_core::player::Player;
use wxdragon::prelude::*;

pub struct MiniPlayer {
    pub frame: Frame,
    pub panel: Panel,
    now_playing: StaticText,
    pub previous: Button,
    pub play: Button,
    pub next: Button,
    pub volume: Slider,
    pub slower: Button,
    pub faster: Button,
    speed: StaticText,
    pub full: Button,
}

impl MiniPlayer {
    pub fn new(player: &Player) -> MiniPlayer {
        let frame = Frame::builder()
            .with_title(&fl!("mini-title"))
            .with_style(
                FrameStyle::Caption
                    | FrameStyle::SystemMenu
                    | FrameStyle::CloseBox
                    | FrameStyle::MinimizeBox
                    | FrameStyle::StayOnTop,
            )
            .build();
        if let Some(icon) = app_icon(32) {
            frame.set_icon(&icon);
        }
        let panel = Panel::builder(&frame).build();
        let now_playing = label(&panel, "");
        let buttons = row();
        let [previous, play, next, slower, faster] =
            [fl!("player-previous"), String::new(), fl!("player-next"), fl!("player-slower"), fl!("player-faster")]
                .map(|text| button(&panel, &text));
        for control in [&previous, &play, &next] {
            buttons.add(control, 0, centered(), 6);
        }
        let speed = label(&panel, "");
        let tempo = row();
        tempo.add(&slower, 0, centered(), 6);
        tempo.add(&speed, 0, centered(), 6);
        tempo.add(&faster, 0, centered(), 6);
        let sizer = column();
        sizer.add(&now_playing, 0, SizerFlag::Expand | SizerFlag::All, 8);
        sizer.add_sizer(&buttons, 0, SizerFlag::All, 6);
        let volume =
            field(&sizer, &panel, &fl!("player-volume"), true, || volume_slider(&panel, player.settings.volume));
        sizer.add_sizer(&tempo, 0, SizerFlag::All, 6);
        let full = button(&panel, &fl!("mini-open-main"));
        sizer.add(&full, 0, SizerFlag::All, 6);
        panel.set_sizer(sizer, true);
        let outer = column();
        outer.add(&panel, 1, SizerFlag::Expand, 0);
        frame.set_sizer_and_fit(outer, true);
        let mini = MiniPlayer { frame, panel, now_playing, previous, play, next, volume, slower, faster, speed, full };
        mini.update_track(player);
        mini.update_playback(player);
        mini.update_modes(player);
        mini
    }
}

impl PlayerView for MiniPlayer {
    fn update_track(&self, player: &Player) {
        self.now_playing.set_label(&view_model::now_playing(player).unwrap_or_else(|| fl!("now-playing-nothing")));
        self.panel.layout();
    }

    fn update_playback(&self, player: &Player) {
        self.play.set_label(&play_label(player));
    }

    fn update_volume(&self, player: &Player) {
        self.volume.set_value(i32::from(player.settings.volume));
    }

    fn update_modes(&self, player: &Player) {
        self.speed.set_label(&speed_text(player));
    }
}
