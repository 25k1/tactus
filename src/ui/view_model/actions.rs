use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemAction {
    Open,
    Play,
    PlayNext,
    AddToQueue,
    Like,
    Unlike,
    Dislike,
    Undislike,
    AddToPlaylist,
    AddToThisPlaylist,
    RemoveFromPlaylist,
    RemoveFromQueue,
    MoveUp,
    MoveDown,
    Download,
    Radio,
    Similar,
    GoToAlbum,
    GoToArtist,
    TrackInfo,
    CopyLink,
    Rename,
    EditDescription,
    Duplicate,
    MakePublic,
    MakePrivate,
    Delete,
    OpenFile,
    ShowFile,
    MarkPlayed,
    MarkUnplayed,
    Pin,
    Unpin,
}

impl ItemAction {
    pub const ALL: [ItemAction; 33] = [
        ItemAction::Open,
        ItemAction::Play,
        ItemAction::PlayNext,
        ItemAction::AddToQueue,
        ItemAction::Like,
        ItemAction::Unlike,
        ItemAction::Dislike,
        ItemAction::Undislike,
        ItemAction::AddToPlaylist,
        ItemAction::AddToThisPlaylist,
        ItemAction::RemoveFromPlaylist,
        ItemAction::RemoveFromQueue,
        ItemAction::MoveUp,
        ItemAction::MoveDown,
        ItemAction::Download,
        ItemAction::Radio,
        ItemAction::Similar,
        ItemAction::GoToAlbum,
        ItemAction::GoToArtist,
        ItemAction::TrackInfo,
        ItemAction::CopyLink,
        ItemAction::Rename,
        ItemAction::EditDescription,
        ItemAction::Duplicate,
        ItemAction::MakePublic,
        ItemAction::MakePrivate,
        ItemAction::Delete,
        ItemAction::OpenFile,
        ItemAction::ShowFile,
        ItemAction::MarkPlayed,
        ItemAction::MarkUnplayed,
        ItemAction::Pin,
        ItemAction::Unpin,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|action| *action == self).unwrap_or_default()
    }

    pub fn label(self) -> String {
        match self {
            ItemAction::Open => fl!("action-open"),
            ItemAction::Play => fl!("action-play"),
            ItemAction::PlayNext => fl!("action-play-next"),
            ItemAction::AddToQueue => fl!("action-add-to-queue"),
            ItemAction::Like => fl!("action-like"),
            ItemAction::Unlike => fl!("action-unlike"),
            ItemAction::Dislike => fl!("action-dislike"),
            ItemAction::Undislike => fl!("action-undislike"),
            ItemAction::AddToPlaylist => fl!("action-add-to-playlist"),
            ItemAction::AddToThisPlaylist => fl!("action-add-to-this-playlist"),
            ItemAction::RemoveFromPlaylist => fl!("action-remove-from-playlist"),
            ItemAction::RemoveFromQueue => fl!("action-remove-from-queue"),
            ItemAction::MoveUp => fl!("action-move-up"),
            ItemAction::MoveDown => fl!("action-move-down"),
            ItemAction::Download => fl!("action-download"),
            ItemAction::Radio => fl!("action-radio"),
            ItemAction::Similar => fl!("action-similar"),
            ItemAction::GoToAlbum => fl!("action-go-to-album"),
            ItemAction::GoToArtist => fl!("action-go-to-artist"),
            ItemAction::TrackInfo => fl!("action-track-info"),
            ItemAction::CopyLink => fl!("action-copy-link"),
            ItemAction::Rename => fl!("action-rename"),
            ItemAction::EditDescription => fl!("action-edit-description"),
            ItemAction::Duplicate => fl!("action-duplicate"),
            ItemAction::MakePublic => fl!("action-make-public"),
            ItemAction::MakePrivate => fl!("action-make-private"),
            ItemAction::Delete => fl!("action-delete"),
            ItemAction::OpenFile => fl!("action-open-file"),
            ItemAction::ShowFile => fl!("action-show-file"),
            ItemAction::MarkPlayed => fl!("action-mark-played"),
            ItemAction::MarkUnplayed => fl!("action-mark-unplayed"),
            ItemAction::Pin => fl!("action-pin"),
            ItemAction::Unpin => fl!("action-unpin"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ListContext {
    pub in_queue: bool,
    pub own_playlist: Option<PlaylistKey>,
    pub section: Option<SectionKind>,
    pub index: usize,
    pub len: usize,
    pub arranged: bool,
}

fn relation_actions(player: &Player, target: &Target, dislike: bool) -> Vec<Option<ItemAction>> {
    let mut actions =
        vec![Some(if player.has(target, Relation::Like) { ItemAction::Unlike } else { ItemAction::Like })];
    if dislike {
        actions.push(Some(if player.has(target, Relation::Dislike) {
            ItemAction::Undislike
        } else {
            ItemAction::Dislike
        }));
    }
    actions
}

fn pin_action(player: &Player, item: &Item) -> Option<ItemAction> {
    let target = item.target()?;
    Some(if player.is_pinned(&target) { ItemAction::Unpin } else { ItemAction::Pin })
}

pub fn item_actions(player: &Player, entry: &Entry, context: &ListContext) -> Vec<Option<ItemAction>> {
    use ItemAction::*;
    let item = match entry {
        Entry::Download(download) => {
            let mut actions = Vec::new();
            if download.file().is_some() {
                actions.extend([Some(Play), Some(OpenFile), Some(ShowFile)]);
            }
            actions.push(Some(CopyLink));
            return actions;
        }
        Entry::Item(item) => item,
    };
    let mut actions = Vec::new();
    match item {
        Item::Track(track) => {
            let movable = !context.arranged;
            if context.in_queue {
                actions.extend([Some(Play), Some(RemoveFromQueue)]);
                if movable && context.index > 0 {
                    actions.push(Some(MoveUp));
                }
                if movable && context.index + 1 < context.len {
                    actions.push(Some(MoveDown));
                }
            } else {
                actions.extend([Some(Play), Some(PlayNext), Some(AddToQueue)]);
            }
            actions.push(None);
            actions.extend(relation_actions(player, &Target::Track(track.clone()), true));
            actions.push(Some(AddToPlaylist));
            match (context.own_playlist, context.section) {
                (Some(_), Some(SectionKind::Suggestions)) => actions.push(Some(AddToThisPlaylist)),
                (Some(_), _) => {
                    actions.push(Some(RemoveFromPlaylist));
                    if movable && context.index > 0 {
                        actions.push(Some(MoveUp));
                    }
                    if movable && context.index + 1 < context.len {
                        actions.push(Some(MoveDown));
                    }
                }
                _ => {}
            }
            actions.push(Some(Download));
            if track.episode {
                actions.push(Some(if track.played { MarkUnplayed } else { MarkPlayed }));
            }
            actions.extend([None, Some(Radio), Some(Similar)]);
            if !track.album_id.is_empty() {
                actions.push(Some(GoToAlbum));
            }
            if !track.artist_id.is_empty() {
                actions.push(Some(GoToArtist));
            }
            actions.extend([Some(TrackInfo), Some(CopyLink)]);
        }
        Item::Album(album) => {
            actions.extend([Some(Open), Some(Play), Some(AddToQueue), None]);
            actions.extend(relation_actions(player, &Target::Album(album.id.clone()), false));
            actions.push(pin_action(player, item));
            actions.extend([Some(Download), Some(Radio)]);
            if !album.artist_id.is_empty() {
                actions.push(Some(GoToArtist));
            }
            actions.push(Some(CopyLink));
        }
        Item::Artist(artist) => {
            actions.extend([Some(Open), Some(Play), Some(Radio), None]);
            actions.extend(relation_actions(player, &Target::Artist(artist.id.clone()), true));
            actions.push(pin_action(player, item));
            actions.push(Some(CopyLink));
        }
        Item::Playlist(playlist) => {
            actions.extend([Some(Open), Some(Play), Some(AddToQueue), Some(Download), Some(Radio), None]);
            if player.is_own(playlist) {
                actions.extend([
                    Some(Rename),
                    Some(EditDescription),
                    Some(if playlist.public { MakePrivate } else { MakePublic }),
                    Some(Delete),
                ]);
            } else {
                actions.extend(relation_actions(player, &Target::Playlist(playlist.key), false));
            }
            actions.extend([pin_action(player, item), Some(Duplicate), Some(CopyLink)]);
        }
        Item::Station(_) => actions.push(Some(Play)),
        Item::Tag(_) | Item::Link(_) => actions.extend([Some(Open), Some(CopyLink)]),
        Item::Genre(genre) if !genre.seed.is_empty() => actions.extend([Some(Open), Some(Radio)]),
        Item::Genre(_) | Item::Category(_) => actions.push(Some(Open)),
        Item::Chart(_) => actions.extend([Some(Open), Some(Play), Some(AddToQueue), Some(CopyLink)]),
    }
    actions
}

fn tidy(actions: Vec<Option<ItemAction>>) -> Vec<Option<ItemAction>> {
    let mut tidy: Vec<Option<ItemAction>> = Vec::with_capacity(actions.len());
    for action in actions {
        if action.is_some() || tidy.last().is_some_and(Option::is_some) {
            tidy.push(action);
        }
    }
    while tidy.last().is_some_and(Option::is_none) {
        tidy.pop();
    }
    tidy
}

pub fn selection_actions(player: &Player, entries: &[Entry], context: &ListContext) -> Vec<Option<ItemAction>> {
    use ItemAction::*;
    let tracks = entries.iter().filter(|entry| entry.track().is_some()).count();
    let targets: Vec<Target> = entries.iter().filter_map(Entry::item).filter_map(Item::target).collect();
    let mut actions = Vec::new();
    if tracks > 0 {
        actions.extend([Some(Play), Some(PlayNext), Some(AddToQueue), None, Some(AddToPlaylist), Some(Download)]);
    }
    if !targets.is_empty() {
        let all = |relation| targets.iter().all(|target| player.has(target, relation));
        actions.extend([None, Some(if all(Relation::Like) { Unlike } else { Like })]);
        if targets.iter().all(|target| matches!(target, Target::Track(_) | Target::Artist(_))) {
            actions.push(Some(if all(Relation::Dislike) { Undislike } else { Dislike }));
        }
    }
    let episodes: Vec<&Track> = entries.iter().filter_map(Entry::track).filter(|track| track.episode).collect();
    if !episodes.is_empty() {
        actions.push(Some(if episodes.iter().all(|track| track.played) { MarkUnplayed } else { MarkPlayed }));
    }
    actions.push(None);
    if tracks > 0 && context.in_queue {
        actions.push(Some(RemoveFromQueue));
    }
    if tracks > 0 && context.own_playlist.is_some() && context.section != Some(SectionKind::Suggestions) {
        actions.push(Some(RemoveFromPlaylist));
    }
    actions.push(Some(CopyLink));
    tidy(actions)
}

pub fn default_action(entry: &Entry) -> Option<ItemAction> {
    Some(match entry {
        Entry::Item(Item::Track(_) | Item::Station(_)) => ItemAction::Play,
        Entry::Item(_) => ItemAction::Open,
        Entry::Download(download) if download.file().is_some() => ItemAction::Play,
        Entry::Download(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_offer_bulk_actions() {
        let player = player();
        let entries: Vec<Entry> = ["1", "2"].map(|id| Entry::Item(Item::Track(track(id)))).to_vec();
        let queue = ListContext { in_queue: true, ..ListContext::default() };
        let actions: Vec<ItemAction> = selection_actions(&player, &entries, &queue).into_iter().flatten().collect();
        assert!(actions.contains(&ItemAction::RemoveFromQueue) && actions.contains(&ItemAction::Like));
        assert!(!actions.contains(&ItemAction::RemoveFromPlaylist));
        let albums = vec![Entry::Item(Item::Album(Album { id: "9".into(), ..Album::default() }))];
        let actions = selection_actions(&player, &albums, &ListContext::default());
        assert_eq!(actions, [Some(ItemAction::Like), None, Some(ItemAction::CopyLink)]);
        assert_eq!(
            tidy(vec![None, Some(ItemAction::Play), None, None, Some(ItemAction::Like), None]),
            [Some(ItemAction::Play), None, Some(ItemAction::Like)]
        );
    }

    #[test]
    fn arranged_lists_cannot_be_reordered() {
        let player = player();
        let queue = ListContext { in_queue: true, index: 1, len: 3, arranged: true, ..ListContext::default() };
        let actions = actions(&player, Item::Track(track("1")), queue);
        assert!(!actions.contains(&ItemAction::MoveUp) && !actions.contains(&ItemAction::MoveDown));
    }

    #[test]
    fn track_actions_depend_on_context() {
        let player = player();
        let normal = actions(&player, Item::Track(track("1")), ListContext::default());
        assert_eq!(&normal[..3], [ItemAction::Play, ItemAction::PlayNext, ItemAction::AddToQueue]);
        assert!(normal.contains(&ItemAction::Like) && normal.contains(&ItemAction::GoToAlbum));
        assert!(!normal.contains(&ItemAction::RemoveFromPlaylist));
        let queue = ListContext { in_queue: true, index: 0, len: 2, ..ListContext::default() };
        let queued = actions(&player, Item::Track(track("1")), queue);
        assert!(queued.contains(&ItemAction::MoveDown) && !queued.contains(&ItemAction::MoveUp));
        let own = ListContext {
            own_playlist: Some(PlaylistKey { owner: 1, kind: 3 }),
            section: Some(SectionKind::Suggestions),
            ..ListContext::default()
        };
        assert!(actions(&player, Item::Track(track("1")), own).contains(&ItemAction::AddToThisPlaylist));
    }

    #[test]
    fn other_items_get_matching_actions() {
        let player = player();
        let album = actions(&player, Item::Album(Album { id: "1".into(), ..Album::default() }), ListContext::default());
        assert_eq!(album[0], ItemAction::Open);
        assert!(!album.contains(&ItemAction::GoToArtist));
        let playlist = actions(&player, Item::Playlist(Playlist::default()), ListContext::default());
        assert!(playlist.contains(&ItemAction::Like) && !playlist.contains(&ItemAction::Delete));
        assert_eq!(ItemAction::ALL.len(), ItemAction::Unpin.index() + 1);
        assert!(album.contains(&ItemAction::Pin));
        let mut episode = track("e");
        episode.episode = true;
        assert!(
            actions(&player, Item::Track(episode.clone()), ListContext::default()).contains(&ItemAction::MarkPlayed)
        );
        episode.played = true;
        assert!(actions(&player, Item::Track(episode), ListContext::default()).contains(&ItemAction::MarkUnplayed));
    }
}
