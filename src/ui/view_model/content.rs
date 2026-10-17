use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Ready,
    Loading,
    Failed(String),
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContentSection {
    pub kind: SectionKind,
    pub label: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Content {
    pub title: String,
    pub description: String,
    pub subject: Option<Item>,
    pub view: Option<View>,
    pub sections: Vec<ContentSection>,
    pub status: Status,
}

impl Default for Content {
    fn default() -> Self {
        Content::new(String::new(), Status::Empty)
    }
}

impl Content {
    fn new(title: String, status: Status) -> Self {
        Self { title, description: String::new(), subject: None, view: None, sections: Vec::new(), status }
    }

    pub fn tracks(&self, section: usize) -> Vec<Track> {
        let tracks = |entries: &[Entry]| entries.iter().filter_map(Entry::track).cloned().collect::<Vec<_>>();
        match self.sections.get(section).map(|section| tracks(&section.entries)) {
            Some(found) if !found.is_empty() => found,
            _ => self
                .sections
                .iter()
                .filter(|section| section.kind == SectionKind::Tracks || section.kind == SectionKind::Episodes)
                .flat_map(|section| tracks(&section.entries))
                .collect(),
        }
    }
}

fn heading(subject: Option<&Item>, title: &str) -> String {
    match subject {
        Some(Item::Album(album)) => fl!("heading-album", title = album.title.clone(), artist = album.artist.clone()),
        Some(Item::Artist(artist)) => fl!("heading-artist", name = artist.name.clone()),
        Some(Item::Playlist(playlist)) => fl!(
            "heading-playlist",
            title = playlist.title.clone(),
            owner = playlist.owner.clone(),
            count = playlist.track_count
        ),
        Some(Item::Track(track)) => fl!("heading-track", title = track.full_title(), artist = track.artist.clone()),
        _ => title.to_owned(),
    }
}

pub fn content(player: &Player, location: &Location, query: &str) -> Content {
    let local = |section: Section, entries: Vec<Entry>| {
        let mut content = Content::new(section.label(), if entries.is_empty() { Status::Empty } else { Status::Ready });
        content.sections.push(ContentSection {
            kind: SectionKind::Tracks,
            label: section_label(SectionKind::Tracks, ""),
            entries,
        });
        content
    };
    match location {
        Location::Section(Section::Queue) => local(
            Section::Queue,
            player
                .queue
                .tracks()
                .iter()
                .cloned()
                .map(|track| Entry::Item(Item::Track(marked(player, track))))
                .collect(),
        ),
        Location::Section(Section::Downloads) => local(Section::Downloads, downloads(player)),
        location => {
            let fallback = location.section().map(Section::label).unwrap_or_default();
            let Some(view) = location.view(query) else {
                return Content::new(fallback, Status::Empty);
            };
            let mut content = match player.page(&view) {
                None | Some(PageState::Loading) => Content::new(fallback, Status::Loading),
                Some(PageState::Failed(failure)) => {
                    Content::new(fallback, Status::Failed(crate::ui::text::failure(failure)))
                }
                Some(PageState::Ready(page)) => {
                    let title = match &view {
                        _ if location.section().is_some() => fallback,
                        View::Chart(region) => chart_label(region),
                        View::User(_) => fl!("heading-user", name = page.title.as_str()),
                        _ => heading(page.subject.as_ref(), &page.title),
                    };
                    let mut content =
                        Content::new(title, if page.sections.is_empty() { Status::Empty } else { Status::Ready });
                    content.description = describe(page);
                    content.subject = page.subject.clone();
                    content.sections = page
                        .sections
                        .iter()
                        .map(|section| ContentSection {
                            kind: section.kind,
                            label: section_label(section.kind, &section.title),
                            entries: section.items.iter().filter_map(|item| shown(player, item)).collect(),
                        })
                        .collect();
                    content
                }
            };
            content.view = Some(view);
            content
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageButton {
    Play,
    Shuffle,
    Radio,
    Like,
    Unlike,
    Download,
    Refresh,
    ClearQueue,
    SaveQueue,
    OpenFolder,
    ClearDownloads,
    ClearSearches,
    HidePlayed,
    ShowPlayed,
    Upload,
}

impl PageButton {
    pub const ALL: [PageButton; 15] = [
        PageButton::Play,
        PageButton::Shuffle,
        PageButton::Radio,
        PageButton::Like,
        PageButton::Unlike,
        PageButton::Download,
        PageButton::Refresh,
        PageButton::ClearQueue,
        PageButton::SaveQueue,
        PageButton::OpenFolder,
        PageButton::ClearDownloads,
        PageButton::ClearSearches,
        PageButton::HidePlayed,
        PageButton::ShowPlayed,
        PageButton::Upload,
    ];

    pub fn label(self) -> String {
        match self {
            PageButton::Play => fl!("button-play"),
            PageButton::Shuffle => fl!("button-shuffle"),
            PageButton::Radio => fl!("button-radio"),
            PageButton::Like => fl!("button-like"),
            PageButton::Unlike => fl!("button-unlike"),
            PageButton::Download => fl!("button-download"),
            PageButton::Refresh => fl!("button-refresh"),
            PageButton::ClearQueue => fl!("button-clear-queue"),
            PageButton::SaveQueue => fl!("button-save-queue"),
            PageButton::OpenFolder => fl!("button-open-folder"),
            PageButton::ClearDownloads => fl!("button-clear-downloads"),
            PageButton::ClearSearches => fl!("button-clear-searches"),
            PageButton::HidePlayed => fl!("button-hide-played"),
            PageButton::ShowPlayed => fl!("button-show-played"),
            PageButton::Upload => fl!("button-upload"),
        }
    }
}

fn marked(player: &Player, mut track: Track) -> Track {
    track.played = player.is_played(&track);
    track.downloaded = player.offline.has(&track.id);
    track
}

fn downloads(player: &Player) -> Vec<Entry> {
    let active: Vec<&str> = player.downloads.iter().map(|download| download.track.id.as_str()).collect();
    let stored = player
        .offline
        .stored()
        .filter(|stored| !active.contains(&stored.track.id.as_str()))
        .map(|stored| Download { id: 0, track: stored.track.clone(), state: DownloadState::Finished(stored.file()) });
    player.downloads.iter().cloned().chain(stored).map(Entry::Download).collect()
}

fn shown(player: &Player, item: &Item) -> Option<Entry> {
    match item {
        Item::Track(track) => {
            let track = marked(player, track.clone());
            (!track.played || !player.settings.hide_played).then_some(Entry::Item(Item::Track(track)))
        }
        item => Some(Entry::Item(item.clone())),
    }
}

fn spoken(content: &Content) -> bool {
    let episodes = content
        .sections
        .iter()
        .flat_map(|section| &section.entries)
        .any(|entry| entry.track().is_some_and(|track| track.episode));
    episodes
        || matches!(&content.subject, Some(Item::Album(album)) if matches!(album.kind, AlbumKind::Podcast | AlbumKind::Audiobook))
}

pub fn buttons(player: &Player, location: &Location, content: &Content, section: usize) -> Vec<PageButton> {
    match location {
        Location::Section(Section::Queue) => return vec![PageButton::ClearQueue, PageButton::SaveQueue],
        Location::Section(Section::Downloads) => return vec![PageButton::OpenFolder, PageButton::ClearDownloads],
        Location::Section(Section::Lyrics) => return Vec::new(),
        Location::Section(Section::Search) if content.view.is_none() => {
            return if player.searches().is_empty() { Vec::new() } else { vec![PageButton::ClearSearches] };
        }
        _ => {}
    }
    let mut buttons = Vec::new();
    if !content.tracks(section).is_empty() {
        buttons.extend([PageButton::Play, PageButton::Shuffle]);
    }
    if let Some(subject) = &content.subject {
        if subject.seed().is_some() {
            buttons.push(PageButton::Radio);
        }
        let own = matches!(subject, Item::Playlist(playlist) if player.is_own(playlist));
        if own {
            buttons.push(PageButton::Upload);
        }
        if let Some(target) = subject.target().filter(|_| !own && !matches!(subject, Item::Track(_))) {
            buttons.push(if player.has(&target, Relation::Like) { PageButton::Unlike } else { PageButton::Like });
        }
    }
    if !content.tracks(section).is_empty() {
        buttons.push(PageButton::Download);
    }
    if spoken(content) {
        buttons.push(if player.settings.hide_played { PageButton::ShowPlayed } else { PageButton::HidePlayed });
    }
    if content.view.is_some() {
        buttons.push(PageButton::Refresh);
    }
    buttons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_sections_show_queue_and_downloads() {
        let mut player = player();
        player.queue.replace(vec![track("1"), track("2")], 0);
        let content = content(&player, &Location::Section(Section::Queue), "");
        assert_eq!(content.sections[0].entries.len(), 2);
        assert_eq!(content.tracks(0).len(), 2);
        assert_eq!(
            buttons(&player, &Location::Section(Section::Queue), &content, 0),
            [PageButton::ClearQueue, PageButton::SaveQueue]
        );
        let search = content_for_empty_search(&player);
        assert_eq!(search.status, Status::Empty);
    }

    fn content_for_empty_search(player: &Player) -> Content {
        content(player, &Location::Section(Section::Search), " ")
    }
}
