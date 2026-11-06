use tactus_api::{Item, Page, Paged, Section, SectionKind, Track, View};
use tactus_winamp::{text, tracks};

pub const PLACES: [(&str, View); 7] = [
    ("nav-wave", View::Waves),
    ("nav-for-you", View::ForYou),
    ("nav-collection", View::Collection),
    ("nav-playlists", View::Playlists),
    ("nav-podcasts", View::Podcasts),
    ("nav-radio", View::Radio),
    ("nav-history", View::History),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub view: View,
    pub title: String,
    pub page: Option<Page>,
    pub section: usize,
    pub item: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Navigation {
    stack: Vec<Place>,
}

impl Navigation {
    pub fn root(&mut self, view: View, title: String) {
        self.stack.clear();
        self.push(view, title);
    }

    pub fn push(&mut self, view: View, title: String) {
        self.stack.push(Place { view, title, page: None, section: 0, item: 0 });
    }

    pub fn back(&mut self) -> bool {
        if self.stack.len() < 2 {
            return false;
        }
        self.stack.pop();
        true
    }

    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn current(&self) -> Option<&Place> {
        self.stack.last()
    }

    pub fn title(&self) -> String {
        self.current().map(|place| place.title.clone()).unwrap_or_default()
    }

    pub fn current_mut(&mut self) -> Option<&mut Place> {
        self.stack.last_mut()
    }

    pub fn loaded(&mut self, view: &View, mut page: Page) -> bool {
        let Some(place) = self.stack.last_mut().filter(|place| place.view == *view) else {
            return false;
        };
        page.sections.retain(|section| !section.items.is_empty());
        if !page.title.is_empty() {
            place.title.clone_from(&page.title);
        }
        place.section = place.section.min(page.sections.len().saturating_sub(1));
        place.page = Some(page);
        true
    }

    pub fn section(&self) -> Option<&Section> {
        let place = self.current()?;
        place.page.as_ref()?.sections.get(place.section)
    }

    pub fn item(&self) -> Option<&Item> {
        self.section()?.items.get(self.current()?.item)
    }

    pub fn select_section(&mut self, index: usize) {
        if let Some(place) = self.current_mut() {
            place.section = index;
            place.item = 0;
        }
    }

    pub fn select_item(&mut self, index: usize) {
        if let Some(place) = self.current_mut() {
            place.item = index;
        }
    }

    pub fn tracks_from_selection(&self) -> (Vec<Track>, usize) {
        let Some(section) = self.section() else {
            return (Vec::new(), 0);
        };
        let item = self.current().map_or(0, |place| place.item);
        let start = section.items[..item.min(section.items.len())].iter().filter(|item| item.track().is_some()).count();
        (section.tracks().cloned().collect(), start)
    }

    pub fn extend(&mut self, view: &View, kind: SectionKind, paged: Paged) -> usize {
        let Some(place) = self.stack.last_mut().filter(|place| place.view == *view) else {
            return 0;
        };
        let Some(section) = place.page.as_mut().and_then(|page| page.sections.iter_mut().find(|s| s.kind == kind))
        else {
            return 0;
        };
        section.extend(paged)
    }
}

pub fn section_names(page: &Page) -> Vec<String> {
    page.sections.iter().map(|section| format!("{} ({})", text::section(section), section.items.len())).collect()
}

pub fn row(item: &Item) -> [String; 3] {
    let length = match item {
        Item::Track(track) if track.duration_ms > 0 => tracks::duration(track.duration_ms),
        _ => String::new(),
    };
    [text::title(item), text::details(item), length]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Item {
        Item::Track(Track { id: id.into(), ..Track::default() })
    }

    fn page() -> Page {
        Page {
            title: "Album".into(),
            sections: vec![
                Section::new(SectionKind::Tracks, "", vec![track("1"), Item::Link(Default::default()), track("2")]),
                Section::new(SectionKind::Similar, "", vec![]),
            ],
            ..Page::default()
        }
    }

    #[test]
    fn keeps_a_back_stack_and_drops_empty_sections() {
        let mut navigation = Navigation::default();
        navigation.root(View::Waves, "Waves".into());
        assert!(!navigation.can_go_back());
        navigation.push(View::Album("5".into()), "…".into());
        assert!(!navigation.loaded(&View::Album("6".into()), page()));
        assert!(navigation.loaded(&View::Album("5".into()), page()));
        let place = navigation.current().unwrap();
        assert_eq!((place.title.as_str(), place.page.as_ref().unwrap().sections.len()), ("Album", 1));
        assert_eq!(navigation.title(), "Album");
        assert!(navigation.back());
        assert_eq!(navigation.current().unwrap().view, View::Waves);
        assert!(!navigation.back());
    }

    #[test]
    fn plays_tracks_from_the_selected_one() {
        let mut navigation = Navigation::default();
        navigation.root(View::Album("5".into()), String::new());
        navigation.loaded(&View::Album("5".into()), page());
        navigation.select_item(2);
        let (tracks, start) = navigation.tracks_from_selection();
        assert_eq!((tracks.len(), start, tracks[start].id.as_str()), (2, 1, "2"));
    }
}
