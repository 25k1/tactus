use crate::error::{Error, Result};
use crate::model::*;
use std::sync::Arc;

pub trait AccountService {
    fn account(&self) -> Result<Account>;

    fn account_settings(&self) -> Result<AccountSettings> {
        Err(Error::Unsupported)
    }

    fn set_account_option(&self, _option: AccountOption, _enabled: bool) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn consume_promo_code(&self, _code: &str) -> Result<String> {
        Err(Error::Unsupported)
    }
}

pub trait CatalogService {
    fn page(&self, view: &View, uid: u64) -> Result<Page>;

    fn more(&self, _view: &View, _kind: SectionKind, _page: u32) -> Result<Paged> {
        Err(Error::Unsupported)
    }

    fn tracks(&self, ids: &[String]) -> Result<Vec<Track>>;

    fn suggestions(&self, _part: &str) -> Result<Suggestions> {
        Err(Error::Unsupported)
    }

    fn track_info(&self, _id: &str) -> Result<TrackInfo> {
        Err(Error::Unsupported)
    }

    fn lyrics(&self, _id: &str) -> Result<Option<Lyrics>> {
        Ok(None)
    }
}

pub trait LibraryService {
    fn library(&self, uid: u64) -> Result<Library>;

    fn set_pin(&self, _target: &Target, _on: bool) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn upload(&self, _uid: u64, _kind: u32, _name: &str, _data: Vec<u8>) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn playlists(&self, uid: u64) -> Result<Vec<Playlist>>;

    fn set_relation(&self, _uid: u64, _target: &Target, _relation: Relation, _on: bool) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn create_playlist(&self, _uid: u64, _title: &str, _public: bool) -> Result<Playlist> {
        Err(Error::Unsupported)
    }

    fn edit_playlist(&self, _uid: u64, _kind: u32, _edit: &PlaylistEdit) -> Result<()> {
        Err(Error::Unsupported)
    }
}

pub trait PlaybackService {
    fn audio_source(&self, _id: &str, _quality: Quality) -> Result<AudioSource> {
        Err(Error::Unsupported)
    }

    fn report_play(&self, _uid: u64, _play: &Play) -> Result<()> {
        Ok(())
    }

    fn start_wave(&self, _seeds: &[String]) -> Result<WaveBatch> {
        Err(Error::Unsupported)
    }

    fn wave_tracks(&self, _session: &str, _queue: &[String]) -> Result<WaveBatch> {
        Err(Error::Unsupported)
    }

    fn wave_feedback(&self, _session: &str, _batch: &str, _event: &WaveEvent) -> Result<()> {
        Ok(())
    }

    fn active_queue(&self) -> Result<Option<RemoteQueue>> {
        Ok(None)
    }

    fn create_queue(&self, _tracks: &[Track], _index: usize, _context: &str) -> Result<String> {
        Err(Error::Unsupported)
    }

    fn update_queue_position(&self, _id: &str, _index: usize) -> Result<()> {
        Ok(())
    }
}

pub trait MusicService: AccountService + CatalogService + LibraryService + PlaybackService + Send + Sync {}

impl<T: AccountService + CatalogService + LibraryService + PlaybackService + Send + Sync> MusicService for T {}

pub type Service = Arc<dyn MusicService>;
