use crate::error::{Error, Result};
use crate::model::*;
use crate::service::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub fn track(id: &str) -> Track {
    Track {
        id: id.to_owned(),
        title: format!("Track {id}"),
        artist: "Artist".into(),
        artist_id: "7".into(),
        album: "Album".into(),
        album_id: "70".into(),
        duration_ms: 180_000,
        ..Track::default()
    }
}

pub fn tracks(ids: &[&str]) -> Vec<Track> {
    ids.iter().map(|id| track(id)).collect()
}

#[derive(Default)]
pub struct FakeService {
    pub pages: HashMap<View, Page>,
    pub library: Library,
    pub calls: Mutex<Vec<String>>,
    demo: bool,
}

fn demo_page(view: &View) -> Page {
    let title = format!("{view:?}");
    let items = (1..=12).map(|index| Item::Track(track(&format!("{index}")))).collect();
    let album = Album { id: "70".into(), title: "Album".into(), artist: "Artist".into(), ..Album::default() };
    Page {
        title: title.clone(),
        sections: vec![
            Section::new(SectionKind::Tracks, title, items).with_more(Some(1)),
            Section::new(SectionKind::Albums, "", vec![Item::Album(album)]),
        ],
        ..Page::default()
    }
}

impl FakeService {
    pub fn demo() -> Self {
        Self { demo: true, ..Self::default() }
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl AccountService for FakeService {
    fn account(&self) -> Result<Account> {
        Ok(Account { uid: 1, login: "tester".into(), name: "Tester".into(), has_plus: true })
    }
}

impl CatalogService for FakeService {
    fn page(&self, view: &View, _uid: u64) -> Result<Page> {
        self.record(format!("page {view:?}"));
        match self.pages.get(view) {
            Some(page) => Ok(page.clone()),
            None if self.demo => Ok(demo_page(view)),
            None => Err(Error::NotFound(format!("{view:?}"))),
        }
    }

    fn more(&self, view: &View, kind: SectionKind, page: u32) -> Result<Paged> {
        self.record(format!("more {view:?} {kind:?} {page}"));
        let ids = [format!("more{page}a"), format!("more{page}b")];
        Ok(Paged { items: ids.iter().map(|id| Item::Track(track(id))).collect(), next: (page < 1).then_some(page + 1) })
    }

    fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        Ok(ids.iter().map(|id| track(id)).collect())
    }
}

impl LibraryService for FakeService {
    fn library(&self, _uid: u64) -> Result<Library> {
        Ok(self.library.clone())
    }

    fn playlists(&self, _uid: u64) -> Result<Vec<Playlist>> {
        Ok(vec![Playlist { key: PlaylistKey { owner: 1, kind: 3 }, title: "Mine".into(), ..Playlist::default() }])
    }

    fn upload(&self, _uid: u64, kind: u32, name: &str, data: Vec<u8>) -> Result<()> {
        self.record(format!("upload {kind} {name} {}", data.len()));
        Ok(())
    }

    fn set_pin(&self, target: &Target, on: bool) -> Result<()> {
        self.record(format!("pin {on} {target:?}"));
        Ok(())
    }

    fn set_relation(&self, _uid: u64, target: &Target, relation: Relation, on: bool) -> Result<()> {
        self.record(format!("{relation:?} {on} {target:?}"));
        Ok(())
    }

    fn create_playlist(&self, _uid: u64, title: &str, public: bool) -> Result<Playlist> {
        self.record(format!("create {title} {public}"));
        Ok(Playlist { key: PlaylistKey { owner: 1, kind: 9 }, title: title.to_owned(), public, ..Playlist::default() })
    }

    fn edit_playlist(&self, _uid: u64, kind: u32, edit: &PlaylistEdit) -> Result<()> {
        self.record(format!("edit {kind} {edit:?}"));
        Ok(())
    }
}

impl PlaybackService for FakeService {
    fn audio_source(&self, id: &str, _quality: Quality) -> Result<AudioSource> {
        Ok(AudioSource { url: format!("fake://{id}"), codec: Codec::Mp3 })
    }

    fn start_wave(&self, seeds: &[String]) -> Result<WaveBatch> {
        self.record(format!("wave {}", seeds.join(",")));
        Ok(WaveBatch { session: "session".into(), batch: "batch".into(), tracks: tracks(&["w1", "w2", "w3"]) })
    }

    fn wave_tracks(&self, session: &str, queue: &[String]) -> Result<WaveBatch> {
        self.record(format!("more {session} {}", queue.len()));
        Ok(WaveBatch { session: session.to_owned(), batch: "batch2".into(), tracks: tracks(&["w4", "w5"]) })
    }
}
