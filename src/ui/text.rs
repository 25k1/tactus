use crate::i18n::fl;
use tactus_api::Relation;
use tactus_api::error::{ErrorKind, Failure};
use tactus_core::player::{Notice, SleepTimer};

pub fn notice(notice: &Notice) -> String {
    match notice {
        Notice::Relation { relation: Relation::Like, on: true } => fl!("notice-liked"),
        Notice::Relation { relation: Relation::Like, on: false } => fl!("notice-unliked"),
        Notice::Relation { relation: Relation::Dislike, on: true } => fl!("notice-disliked"),
        Notice::Relation { relation: Relation::Dislike, on: false } => fl!("notice-undisliked"),
        Notice::PlaylistCreated(title) => fl!("notice-playlist-created", title = title.as_str()),
        Notice::PlaylistRenamed => fl!("notice-playlist-renamed"),
        Notice::PlaylistDescribed => fl!("notice-playlist-described"),
        Notice::MovedInPlaylist => fl!("notice-moved-in-playlist"),
        Notice::PlaylistDeleted => fl!("notice-playlist-deleted"),
        Notice::PlaylistVisibility(true) => fl!("notice-playlist-public"),
        Notice::PlaylistVisibility(false) => fl!("notice-playlist-private"),
        Notice::AddedToPlaylist { title, count } => {
            fl!("notice-added-to-playlist", title = title.as_str(), count = (*count))
        }
        Notice::RemovedFromPlaylist(count) => fl!("notice-removed-from-playlist", count = (*count)),
        Notice::PlayNext(count) => fl!("notice-play-next", count = (*count)),
        Notice::Queued(count) => fl!("notice-queued", count = (*count)),
        Notice::RemovedFromQueue(count) => fl!("notice-removed-from-queue", count = (*count)),
        Notice::QueueCleared => fl!("notice-queue-cleared"),
        Notice::DownloadQueued(count) => fl!("notice-download-queued", count = (*count)),
        Notice::Downloaded(title) => fl!("notice-downloaded", title = title.as_str()),
        Notice::DownloadFailed { title, error } => {
            fl!("notice-download-failed", title = title.as_str(), error = error.as_str())
        }
        Notice::WaveStarted => fl!("notice-wave-started"),
        Notice::SearchResults(count) => fl!("notice-search-results", count = (*count)),
        Notice::AccountUpdated => fl!("notice-account-updated"),
        Notice::MoreLoaded(count) => fl!("notice-more-loaded", count = (*count)),
        Notice::Promo(status) => status.clone(),
        Notice::Sleep(None) => fl!("notice-sleep-off"),
        Notice::Sleep(Some(SleepTimer::Minutes(minutes))) => fl!("notice-sleep-minutes", minutes = (*minutes)),
        Notice::Sleep(Some(SleepTimer::EndOfTrack)) => fl!("notice-sleep-end-of-track"),
        Notice::Sleep(Some(SleepTimer::EndOfQueue)) => fl!("notice-sleep-end-of-queue"),
        Notice::SleepSoon => fl!("notice-sleep-soon"),
        Notice::SleepStopped => fl!("notice-sleep-stopped"),
        Notice::UpToDate => fl!("notice-up-to-date"),
        Notice::Undone => fl!("notice-undone"),
        Notice::Uploaded(count) => fl!("notice-uploaded", count = (*count)),
        Notice::VoiceReady => fl!("notice-voice-ready"),
        Notice::VoiceRemoved => fl!("notice-voice-removed"),
        Notice::VoiceCancelled => fl!("notice-voice-cancelled"),
        Notice::Offline => fl!("notice-offline"),
        Notice::Online => fl!("notice-online"),
        Notice::Pinned(true) => fl!("notice-pinned"),
        Notice::Pinned(false) => fl!("notice-unpinned"),
        Notice::Played { count, played: true } => fl!("notice-played", count = (*count)),
        Notice::Played { count, played: false } => fl!("notice-unplayed", count = (*count)),
        Notice::NothingToUndo => fl!("notice-nothing-to-undo"),
    }
}

pub fn failure(failure: &Failure) -> String {
    match failure.kind {
        ErrorKind::Network => fl!("error-network"),
        ErrorKind::SignedOut => fl!("error-signed-out"),
        ErrorKind::Expired => fl!("error-expired"),
        ErrorKind::NotFound => fl!("error-not-found"),
        ErrorKind::Subscription => fl!("error-subscription"),
        ErrorKind::Region => fl!("error-region"),
        ErrorKind::RateLimited => fl!("error-rate-limited"),
        ErrorKind::Server => fl!("error-server"),
        ErrorKind::Audio => fl!("error-audio"),
        ErrorKind::Storage => fl!("error-storage"),
        ErrorKind::Unsupported => fl!("error-unsupported"),
        ErrorKind::Other => fl!("error-other", detail = failure.detail.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_has_a_message() {
        let kinds = [
            ErrorKind::Network,
            ErrorKind::SignedOut,
            ErrorKind::Expired,
            ErrorKind::NotFound,
            ErrorKind::Subscription,
            ErrorKind::Region,
            ErrorKind::RateLimited,
            ErrorKind::Server,
            ErrorKind::Audio,
            ErrorKind::Storage,
            ErrorKind::Unsupported,
        ];
        for kind in kinds {
            let text = failure(&Failure { kind, detail: "raw detail".into() });
            assert!(!text.is_empty() && !text.contains("raw detail"), "{kind:?}");
        }
        assert!(failure(&Failure { kind: ErrorKind::Other, detail: "raw detail".into() }).contains("raw detail"));
    }

    #[test]
    fn notices_include_their_details() {
        assert!(notice(&Notice::Downloaded("Song".into())).contains("Song"));
        assert!(!notice(&Notice::QueueCleared).is_empty());
    }
}
