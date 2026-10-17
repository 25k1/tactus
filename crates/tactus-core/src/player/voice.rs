use super::*;
use crate::voice::{self, Artifact, ModelSize, RUNTIME};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

impl Player {
    pub fn installing_voice(&self) -> bool {
        self.voice_install.is_some()
    }

    pub fn install_voice(&mut self, language: &str, size: ModelSize) {
        if self.voice_install.is_some() {
            return;
        }
        let parts: Vec<Artifact> =
            [RUNTIME, voice::model(language, size)].into_iter().filter(|artifact| !artifact.installed()).collect();
        if parts.is_empty() {
            return self.notice(Notice::VoiceReady);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.voice_install = Some(cancel.clone());
        let sender = self.sender.clone();
        let total: u64 = parts.iter().map(|artifact| artifact.size).sum();
        self.emit(Event::VoiceProgress(0));
        self.background(
            move || {
                let (offset, reported) = (AtomicU64::new(0), AtomicU64::new(0));
                for artifact in &parts {
                    let base = offset.load(Ordering::Relaxed);
                    voice::install(artifact, &cancel, |done, _| {
                        let percent = ((base + done) * 100 / total.max(1)).min(100);
                        if reported.swap(percent, Ordering::Relaxed) != percent {
                            let _ = sender.send(Box::new(move |player: &mut Player| {
                                player.emit(Event::VoiceProgress(percent as u8));
                            }));
                        }
                    })?;
                    offset.fetch_add(artifact.size, Ordering::Relaxed);
                }
                Ok(())
            },
            |player, result| {
                let cancelled = player.voice_install.take().is_some_and(|cancel| cancel.load(Ordering::Relaxed));
                match result {
                    Ok(()) => player.notice(Notice::VoiceReady),
                    Err(_) if cancelled => player.notice(Notice::VoiceCancelled),
                    Err(error) => player.fail(error),
                }
            },
        );
    }

    pub fn cancel_voice_install(&mut self) {
        if let Some(cancel) = &self.voice_install {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn remove_voice_model(&mut self, language: &str, size: ModelSize) {
        match voice::remove(&voice::model(language, size)) {
            Ok(()) => self.notice(Notice::VoiceRemoved),
            Err(error) => self.fail(error),
        }
    }
}
