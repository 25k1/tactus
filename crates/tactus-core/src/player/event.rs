use super::SleepTimer;
use crate::announce::Announcement;
use tactus_api::error::Failure;
use tactus_api::{Relation, Suggestions, Track, TrackInfo, View};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Like,
    Unlike,
    Dislike,
    Added,
    Done,
    Error,
    Wave,
    Listening,
    Understood,
    NotUnderstood,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    Relation { relation: Relation, on: bool },
    PlaylistCreated(String),
    PlaylistRenamed,
    PlaylistDescribed,
    MovedInPlaylist,
    PlaylistDeleted,
    PlaylistVisibility(bool),
    AddedToPlaylist { title: String, count: usize },
    RemovedFromPlaylist(usize),
    PlayNext(usize),
    Queued(usize),
    RemovedFromQueue(usize),
    QueueCleared,
    DownloadQueued(usize),
    Downloaded(String),
    DownloadFailed { title: String, error: String },
    WaveStarted,
    SearchResults(usize),
    MoreLoaded(usize),
    Sleep(Option<SleepTimer>),
    SleepSoon,
    SleepStopped,
    UpToDate,
    Undone,
    NothingToUndo,
    Played { count: usize, played: bool },
    Pinned(bool),
    Uploaded(usize),
    Offline,
    Online,
    VoiceReady,
    VoiceRemoved,
    VoiceCancelled,
    AccountUpdated,
    Promo(String),
}

impl Notice {
    pub fn announcement(&self) -> Announcement {
        match self {
            Notice::Relation { .. } => Announcement::Likes,
            Notice::PlaylistCreated(_)
            | Notice::PlaylistRenamed
            | Notice::PlaylistDescribed
            | Notice::MovedInPlaylist
            | Notice::PlaylistDeleted
            | Notice::PlaylistVisibility(_)
            | Notice::AddedToPlaylist { .. }
            | Notice::RemovedFromPlaylist(_)
            | Notice::AccountUpdated
            | Notice::Pinned(_)
            | Notice::Uploaded(_)
            | Notice::Promo(_) => Announcement::Playlists,
            Notice::PlayNext(_)
            | Notice::Queued(_)
            | Notice::RemovedFromQueue(_)
            | Notice::Undone
            | Notice::Played { .. }
            | Notice::QueueCleared
            | Notice::WaveStarted => Announcement::Queue,
            Notice::DownloadQueued(_)
            | Notice::Downloaded(_)
            | Notice::DownloadFailed { .. }
            | Notice::VoiceReady
            | Notice::VoiceRemoved
            | Notice::VoiceCancelled => Announcement::Downloads,
            Notice::SearchResults(_) | Notice::MoreLoaded(_) => Announcement::Search,
            Notice::Sleep(_) | Notice::SleepSoon | Notice::SleepStopped => Announcement::Playback,
            Notice::UpToDate | Notice::NothingToUndo | Notice::Offline | Notice::Online => Announcement::Errors,
        }
    }

    pub fn cue(&self) -> Option<Cue> {
        Some(match self {
            Notice::Relation { relation: Relation::Like, on: true } => Cue::Like,
            Notice::Relation { relation: Relation::Dislike, on: true } => Cue::Dislike,
            Notice::Relation { on: false, .. } => Cue::Unlike,
            Notice::AddedToPlaylist { .. } | Notice::PlayNext(_) | Notice::Queued(_) => Cue::Added,
            Notice::Downloaded(_) => Cue::Done,
            Notice::DownloadFailed { .. } => Cue::Error,
            Notice::WaveStarted => Cue::Wave,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Login,
    Account,
    Track,
    Playback,
    Position,
    Volume,
    Modes,
    Queue,
    Library,
    Playlists,
    Page(View),
    Open(View),
    Downloads,
    Lyrics,
    LyricLine(usize),
    Busy,
    Notice(Notice),
    Error(Failure),
    Suggestions(String, Suggestions),
    TrackInfo(Box<TrackInfo>),
    AccountSettings,
    Searches,
    Devices,
    VoiceProgress(u8),
    NewEpisodes(Vec<Track>),
    UpdateAvailable(Box<crate::update::Release>),
    Updated(std::path::PathBuf),
    Raise,
    Quit,
}

impl Event {
    pub fn repeatable(&self) -> bool {
        matches!(self, Event::Notice(_) | Event::Error(_) | Event::LyricLine(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_have_categories_and_cues() {
        let liked = Notice::Relation { relation: Relation::Like, on: true };
        assert_eq!(liked.announcement(), Announcement::Likes);
        assert_eq!(liked.cue(), Some(Cue::Like));
        let downloaded = Notice::Downloaded("Song".into());
        assert_eq!(downloaded.announcement(), Announcement::Downloads);
        assert_eq!(downloaded.cue(), Some(Cue::Done));
        assert_eq!(Notice::QueueCleared.cue(), None);
    }
}
