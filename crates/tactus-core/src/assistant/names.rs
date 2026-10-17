use super::*;
use crate::player::PageState;
use tactus_api::{Section, SectionKind, Track};
use tactus_voice::Pick;

fn fits(kind: NameKind, item: &Item) -> bool {
    matches!(
        (kind, item),
        (NameKind::Any, _)
            | (NameKind::Artist, Item::Artist(_))
            | (NameKind::Album, Item::Album(_))
            | (NameKind::Playlist, Item::Playlist(_))
            | (NameKind::Track, Item::Track(_))
    )
}

pub(super) fn choose(name: &str, titles: impl IntoIterator<Item = String>) -> Pick {
    let titles: Vec<String> = titles.into_iter().collect();
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    tactus_voice::pick(&tactus_voice::best_matches(name, &titles))
}

fn library(player: &Player) -> Vec<Item> {
    let mut items: Vec<Item> = player.playlists.iter().cloned().map(Item::Playlist).collect();
    if let Some(PageState::Ready(page)) = player.page(&View::Collection) {
        let liked =
            |section: &&Section| !matches!(section.kind, SectionKind::DislikedTracks | SectionKind::DislikedArtists);
        items.extend(page.sections.iter().filter(liked).flat_map(|section| section.items.iter().cloned()));
    }
    items
}

fn likes(player: &Player) -> Option<Vec<Track>> {
    match player.page(&View::Collection) {
        Some(PageState::Ready(page)) => Some(
            page.sections
                .iter()
                .filter(|section| section.kind == SectionKind::Tracks)
                .flat_map(Section::tracks)
                .cloned()
                .collect(),
        ),
        _ => None,
    }
}

fn results(player: &Player, view: &View, kind: NameKind) -> Vec<Item> {
    match player.page(view) {
        Some(PageState::Ready(page)) => page
            .sections
            .iter()
            .flat_map(|section| section.items.iter().take(RESULTS))
            .filter(|item| fits(kind, item))
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn play(player: &mut Player, item: Item) {
    match item {
        Item::Track(track) => player.play_tracks(vec![track], 0, "voice"),
        Item::Station(station) => player.start_wave(vec![station.seed]),
        item => {
            if let Some(view) = View::of(&item) {
                player.play_view(view);
            }
        }
    }
}

impl Assistant {
    fn confirm(&mut self, item: Item, now: Instant) -> Outcome {
        let title = item.title();
        self.pending = Some((item, now + CONFIRM));
        Outcome::Reply(Reply::WillPlay(title))
    }

    pub(super) fn play_name(&mut self, player: &mut Player, kind: NameKind, name: String, now: Instant) -> Outcome {
        let mut items: Vec<Item> = library(player).into_iter().filter(|item| fits(kind, item)).collect();
        if let Pick::One(index) = choose(&name, items.iter().map(Item::title)) {
            return self.confirm(items.swap_remove(index), now);
        }
        let view = View::Search(name.clone());
        player.load(&view, true);
        self.awaiting = Some(Awaiting::Search { view, kind, name: name.clone() });
        Outcome::Reply(Reply::Searching(name))
    }

    pub(super) fn play_likes(&mut self, player: &mut Player) -> Option<Outcome> {
        match likes(player) {
            Some(tracks) if tracks.is_empty() => Some(Outcome::Reply(Reply::NoLikes)),
            Some(tracks) => {
                player.play_tracks(tracks, 0, "likes");
                None
            }
            None => {
                self.awaiting = Some(Awaiting::Likes);
                player.load(&View::Collection, false);
                None
            }
        }
    }

    pub fn waits_for(&self, view: &View) -> bool {
        match &self.awaiting {
            Some(Awaiting::Search { view: wanted, .. }) => wanted == view,
            Some(Awaiting::Likes) => *view == View::Collection,
            None => false,
        }
    }

    pub fn page_ready(&mut self, player: &mut Player, view: &View, now: Instant) -> Option<Outcome> {
        if !self.waits_for(view) {
            return None;
        }
        match player.page(view) {
            Some(PageState::Ready(_)) => {}
            Some(PageState::Failed(_)) => {
                self.awaiting = None;
                return None;
            }
            _ => return None,
        }
        match self.awaiting.take()? {
            Awaiting::Likes => self.play_likes(player),
            Awaiting::Search { kind, name, .. } => {
                let mut items = results(player, view, kind);
                Some(match choose(&name, items.iter().map(Item::title)) {
                    Pick::One(index) => self.confirm(items.swap_remove(index), now),
                    Pick::Nothing if !items.is_empty() => self.confirm(items.swap_remove(0), now),
                    Pick::Nothing => Outcome::Reply(Reply::NotFound(name)),
                    Pick::Several(_) => Outcome::Several(name),
                })
            }
        }
    }
}
