use tactus_core::player::Cue;
use wxdragon::prelude::*;

pub struct Sounds {
    cues: Vec<(Cue, Sound)>,
}

impl Sounds {
    pub fn new() -> Self {
        let cues = [
            (Cue::Like, &include_bytes!("../../assets/sounds/like.wav")[..]),
            (Cue::Unlike, &include_bytes!("../../assets/sounds/unlike.wav")[..]),
            (Cue::Dislike, &include_bytes!("../../assets/sounds/dislike.wav")[..]),
            (Cue::Added, &include_bytes!("../../assets/sounds/added.wav")[..]),
            (Cue::Done, &include_bytes!("../../assets/sounds/done.wav")[..]),
            (Cue::Error, &include_bytes!("../../assets/sounds/error.wav")[..]),
            (Cue::Wave, &include_bytes!("../../assets/sounds/wave.wav")[..]),
            (Cue::Listening, &include_bytes!("../../assets/sounds/listening.wav")[..]),
            (Cue::Understood, &include_bytes!("../../assets/sounds/understood.wav")[..]),
            (Cue::NotUnderstood, &include_bytes!("../../assets/sounds/not-understood.wav")[..]),
        ];
        Self { cues: cues.into_iter().map(|(cue, data)| (cue, Sound::from_data(data))).collect() }
    }

    pub fn play(&self, cue: Cue) {
        if let Some((_, sound)) = self.cues.iter().find(|(candidate, _)| *candidate == cue) {
            sound.play(SoundFlags::Async);
        }
    }
}
