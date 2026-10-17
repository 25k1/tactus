use super::*;
use tactus_core::announce::TRACK_FORMATS;

pub(super) struct Speech {
    announcements: CheckListBox,
    track_format: ComboBox,
    cues: CheckBox,
}

fn announcement_label(kind: Announcement) -> String {
    match kind {
        Announcement::Track => fl!("announce-kind-track"),
        Announcement::Playback => fl!("announce-kind-playback"),
        Announcement::Volume => fl!("announce-kind-volume"),
        Announcement::Seek => fl!("announce-kind-seek"),
        Announcement::Modes => fl!("announce-kind-modes"),
        Announcement::Likes => fl!("announce-kind-likes"),
        Announcement::Queue => fl!("announce-kind-queue"),
        Announcement::Playlists => fl!("announce-kind-playlists"),
        Announcement::Downloads => fl!("announce-kind-downloads"),
        Announcement::Search => fl!("announce-kind-search"),
        Announcement::Lyrics => fl!("announce-kind-lyrics"),
        Announcement::Errors => fl!("announce-kind-errors"),
        Announcement::Voice => fl!("announce-kind-voice"),
    }
}

impl Speech {
    pub(super) fn new(notebook: &Notebook, old: &Settings, backend: Option<String>) -> Self {
        let (panel, sizer) = page(notebook, &fl!("settings-speech"));
        add(&sizer, &label(&panel, &fl!("settings-announce")));
        let announcements = CheckListBox::builder(&panel)
            .with_choices(Announcement::ALL.into_iter().map(announcement_label).collect())
            .build();
        announcements.set_accessibility_label(&fl!("settings-announce"));
        for (index, kind) in Announcement::ALL.into_iter().enumerate() {
            announcements.check(index as u32, old.announcements.contains(&kind));
        }
        sizer.add(&announcements, 1, SizerFlag::Expand | SizerFlag::All, 6);
        let track_format = field(&sizer, &panel, &fl!("settings-track-format"), true, || {
            editable(&panel, &TRACK_FORMATS, &old.track_format)
        });
        add(&sizer, &label(&panel, &fl!("settings-track-format-hint")));
        let cues = check(&panel, &fl!("settings-sound-cues"), old.sound_cues);
        add(&sizer, &cues);
        let engine = match backend {
            Some(name) => fl!("settings-speech-engine", backend = name),
            None => fl!("settings-speech-unavailable"),
        };
        add(&sizer, &label(&panel, &engine));
        panel.set_sizer(sizer, true);
        Self { announcements, track_format, cues }
    }
}

impl Tab for Speech {
    fn apply(&self, settings: &mut Settings) {
        settings.announcements = Announcement::ALL
            .into_iter()
            .enumerate()
            .filter(|(index, _)| self.announcements.is_checked(*index as u32))
            .map(|(_, kind)| kind)
            .collect();
        settings.track_format = self.track_format.get_value().trim().to_owned();
        settings.sound_cues = self.cues.is_checked();
    }
}
