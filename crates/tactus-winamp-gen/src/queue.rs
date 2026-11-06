use crate::session::Session;
use tactus_api::Track;
use tactus_api::error::Result;
use tactus_winamp::bridge::{self, INPUT_MODULE, REMEMBER_TRACKS, RememberTracks};
use tactus_winamp::sdk::COMMAND_PLAY;
use tactus_winamp::{speech, text, tracks};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Replace(usize),
    Append,
    Next,
}

fn remember(tracks: &[Track]) -> bool {
    let remember = unsafe { bridge::function::<RememberTracks>(INPUT_MODULE, REMEMBER_TRACKS) };
    let Some(remember) = remember else {
        return false;
    };
    if let Ok(data) = serde_json::to_vec(tracks) {
        unsafe { remember(data.as_ptr(), data.len()) };
    }
    true
}

fn enqueue(session: &Session, tracks: &[Track]) -> Result<()> {
    let playlist = session.store().save_playlist(&tracks::playlist(tracks))?;
    session.winamp.enqueue(&playlist.to_string_lossy());
    Ok(())
}

fn replace(session: &Session, tracks: &[Track], start: usize) -> Result<()> {
    session.state().wave = None;
    session.winamp.clear();
    enqueue(session, tracks)?;
    session.winamp.select(start.min(tracks.len() - 1));
    session.winamp.command(COMMAND_PLAY);
    Ok(())
}

fn insert_next(session: &Session, tracks: &[Track]) {
    let winamp = session.winamp;
    let at = if winamp.is_empty() { 0 } else { winamp.index() + 1 };
    for (offset, track) in tracks.iter().enumerate() {
        winamp.insert(at + offset, &tracks::uri(track));
    }
}

pub fn put(session: &Session, tracks: &[Track], placement: Placement) -> Result<()> {
    if tracks.is_empty() {
        return Ok(());
    }
    if !remember(tracks) {
        session.winamp.message(&text::text("winamp-input-missing"));
        return Ok(());
    }
    match placement {
        Placement::Replace(start) => return replace(session, tracks, start),
        Placement::Append => enqueue(session, tracks)?,
        Placement::Next => insert_next(session, tracks),
    }
    speech::say(text::text_with("winamp-queued", &[("count", &tracks.len().to_string())]));
    Ok(())
}

pub fn extend(session: &Session, tracks: &[Track]) -> Result<()> {
    if tracks.is_empty() || !remember(tracks) {
        return Ok(());
    }
    enqueue(session, tracks)
}
