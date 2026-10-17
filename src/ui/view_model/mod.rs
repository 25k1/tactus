use super::navigation::{Location, Section};
use crate::i18n::fl;
use tactus_api::{AlbumKind, Artist, Item, Page, PlaylistKey, Relation, SectionKind, Target, Track, TrackInfo, View};
use tactus_core::announce::format_time;
use tactus_core::download::{Download, DownloadState};
use tactus_core::player::{PageState, Player, SleepTimer};

mod actions;
mod content;
mod details;
mod labels;
mod list;

pub use actions::*;
pub use content::*;
pub use details::*;
pub use labels::*;
pub use list::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item(Item),
    Download(Download),
}

impl Entry {
    pub fn item(&self) -> Option<&Item> {
        match self {
            Entry::Item(item) => Some(item),
            Entry::Download(_) => None,
        }
    }

    pub fn track(&self) -> Option<&Track> {
        match self {
            Entry::Item(Item::Track(track)) => Some(track),
            Entry::Download(download) => Some(&download.track),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub origin: usize,
    pub entry: Entry,
}

pub fn join_present(parts: impl IntoIterator<Item = String>, separator: &str) -> String {
    parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(separator)
}

pub fn windows_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

pub fn item_title(item: &Item) -> Option<String> {
    Some(match item {
        Item::Chart(chart) => chart_label(&chart.region),
        item => item.title(),
    })
    .filter(|title| !title.is_empty())
}

#[cfg(test)]
use testing::*;

#[cfg(test)]
mod testing {
    use super::*;
    pub use tactus_api::fake::track;
    pub use tactus_api::{Album, Playlist};
    pub use tactus_core::settings::Settings;

    pub fn player() -> Player {
        Player::new(Settings::default(), |_| Err(tactus_api::error::Error::Unsupported))
    }

    pub fn actions(player: &Player, item: Item, context: ListContext) -> Vec<ItemAction> {
        item_actions(player, &Entry::Item(item), &context).into_iter().flatten().collect()
    }
}
