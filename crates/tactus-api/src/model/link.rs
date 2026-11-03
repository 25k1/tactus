use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaylistLink {
    Owner { owner: String, kind: u32 },
    Shared(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    View(View),
    Playlist(PlaylistLink),
    Join { playlist: PlaylistLink, token: String },
}
