use super::widgets::*;
use crate::i18n::fl;
use tactus_core::player::Player;
use wxdragon::prelude::*;

pub struct LyricsPanel {
    pub panel: Panel,
    heading: StaticText,
    pub lines: ListBox,
}

impl LyricsPanel {
    pub fn new(parent: &impl WxWidget) -> LyricsPanel {
        let panel = Panel::builder(parent).build();
        let heading = label(&panel, &fl!("lyrics-empty"));
        let lines = ListBox::builder(&panel).build();
        lines.set_accessibility_label(&fl!("nav-lyrics"));
        let sizer = column();
        sizer.add(&heading, 0, SizerFlag::Expand | SizerFlag::All, 8);
        sizer.add(&lines, 1, SizerFlag::Expand | SizerFlag::All, 8);
        panel.set_sizer(sizer, true);
        LyricsPanel { panel, heading, lines }
    }

    pub fn update(&self, player: &Player) {
        let heading = match (&player.track, &player.lyrics) {
            (None, _) => fl!("lyrics-empty"),
            (Some(track), None) => fl!("lyrics-unavailable", title = track.full_title()),
            (Some(track), Some(lyrics)) if lyrics.synced() => fl!("lyrics-synced", title = track.full_title()),
            (Some(track), Some(_)) => fl!("lyrics-plain", title = track.full_title()),
        };
        self.heading.set_label(&heading);
        self.lines.clear();
        if let Some(lyrics) = &player.lyrics {
            let lines: Vec<&str> = lyrics.lines.iter().map(|line| line.text.as_str()).collect();
            self.lines.append_items(&lines);
            if !lines.is_empty() {
                self.lines.set_selection(0, true);
            }
        }
        self.lines.set_accessibility_label(&heading);
    }

    pub fn follow(&self, line: usize) {
        if (line as u32) < self.lines.get_count() {
            self.lines.set_selection(line as u32, true);
            self.lines.ensure_visible(line as i32);
        }
    }
}
