use crate::i18n::fl;
use tactus_api::View;

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Search,
    Wave,
    ForYou,
    Collection,
    Playlists,
    Radio,
    Podcasts,
    History,
    Queue,
    Lyrics,
    Downloads,
}

impl Section {
    pub const ALL: [Section; 11] = [
        Section::Search,
        Section::Wave,
        Section::ForYou,
        Section::Collection,
        Section::Playlists,
        Section::Radio,
        Section::Podcasts,
        Section::History,
        Section::Queue,
        Section::Lyrics,
        Section::Downloads,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|section| *section == self).unwrap_or_default()
    }

    pub fn label(self) -> String {
        match self {
            Section::Search => fl!("nav-search"),
            Section::Wave => fl!("nav-wave"),
            Section::ForYou => fl!("nav-for-you"),
            Section::Collection => fl!("nav-collection"),
            Section::Playlists => fl!("nav-playlists"),
            Section::Radio => fl!("nav-radio"),
            Section::Podcasts => fl!("nav-podcasts"),
            Section::History => fl!("nav-history"),
            Section::Queue => fl!("nav-queue"),
            Section::Lyrics => fl!("nav-lyrics"),
            Section::Downloads => fl!("nav-downloads"),
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Section::Search => "Ctrl+1",
            Section::Wave => "Ctrl+2",
            Section::ForYou => "Ctrl+3",
            Section::Collection => "Ctrl+4",
            Section::Playlists => "Ctrl+5",
            Section::Radio => "Ctrl+6",
            Section::Podcasts => "Ctrl+7",
            Section::History => "Ctrl+8",
            Section::Queue => "Ctrl+9",
            Section::Lyrics => "Ctrl+0",
            Section::Downloads => "Ctrl+J",
        }
    }

    pub fn view(self, query: &str) -> Option<View> {
        Some(match self {
            Section::Search if query.trim().is_empty() => return None,
            Section::Search => View::Search(query.trim().to_owned()),
            Section::Wave => View::Waves,
            Section::ForYou => View::ForYou,
            Section::Collection => View::Collection,
            Section::Playlists => View::Playlists,
            Section::Radio => View::Radio,
            Section::Podcasts => View::Podcasts,
            Section::History => View::History,
            Section::Queue | Section::Lyrics | Section::Downloads => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    Section(Section),
    View(View),
}

impl Location {
    pub fn section(&self) -> Option<Section> {
        match self {
            Location::Section(section) => Some(*section),
            Location::View(_) => None,
        }
    }

    pub fn view(&self, query: &str) -> Option<View> {
        match self {
            Location::Section(section) => section.view(query),
            Location::View(view) => Some(view.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spot {
    pub section: usize,
    pub row: usize,
    pub filter: String,
    pub sort: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visit {
    pub location: Location,
    pub spot: Spot,
}

#[derive(Debug, Default)]
pub struct History {
    visits: Vec<Visit>,
    index: usize,
}

impl History {
    pub fn current(&self) -> Option<&Visit> {
        self.visits.get(self.index)
    }

    pub fn remember(&mut self, spot: Spot) {
        if let Some(visit) = self.visits.get_mut(self.index) {
            visit.spot = spot;
        }
    }

    pub fn push(&mut self, location: Location) -> &Visit {
        if self.current().is_none_or(|visit| visit.location != location) {
            self.visits.truncate(self.index + 1);
            self.visits.push(Visit { location, spot: Spot::default() });
            if self.visits.len() > HISTORY_LIMIT {
                self.visits.remove(0);
            }
            self.index = self.visits.len() - 1;
        }
        &self.visits[self.index]
    }

    pub fn back(&mut self) -> Option<&Visit> {
        self.index = self.index.checked_sub(1)?;
        self.visits.get(self.index)
    }

    pub fn forward(&mut self) -> Option<&Visit> {
        if self.index + 1 >= self.visits.len() {
            return None;
        }
        self.index += 1;
        self.visits.get(self.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigates_back_and_forward_remembering_rows() {
        let mut history = History::default();
        history.push(Location::Section(Section::Search));
        let spot = Spot { section: 1, row: 7, filter: "abba".into(), sort: 2 };
        history.remember(spot.clone());
        history.push(Location::View(View::Album("1".into())));
        history.push(Location::View(View::Album("1".into())));
        assert_eq!(history.back().unwrap().spot, spot);
        assert!(history.back().is_none());
        assert_eq!(history.forward().unwrap().location, Location::View(View::Album("1".into())));
        assert!(history.forward().is_none());
        history.back();
        history.push(Location::Section(Section::Radio));
        assert!(history.forward().is_none());
    }

    #[test]
    fn sections_map_to_views() {
        assert_eq!(Section::Search.view("  "), None);
        assert_eq!(Section::Search.view(" abba "), Some(View::Search("abba".into())));
        assert_eq!(Section::Queue.view(""), None);
        assert_eq!(Section::ALL.len(), 11);
        assert_eq!(Section::Downloads.index(), 10);
    }
}
