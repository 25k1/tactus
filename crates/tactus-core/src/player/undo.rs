use super::*;

const UNDO_LIMIT: usize = 20;

pub type Change = (Target, Relation, bool);

#[derive(Clone, Debug, PartialEq)]
pub enum Undo {
    Queue(Vec<(usize, Track)>),
    Playlist(Playlist, Vec<(usize, Track)>),
    Relations(Vec<Change>),
}

impl Player {
    pub(super) fn remember_undo(&mut self, undo: Undo) {
        self.undo.push(undo);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) {
        match self.undo.pop() {
            None => self.notice(Notice::NothingToUndo),
            Some(Undo::Queue(removed)) => {
                for (index, track) in removed {
                    self.queue.insert_at(index, track);
                }
                self.emit(Event::Queue);
                self.notice(Notice::Undone);
            }
            Some(Undo::Playlist(playlist, removed)) => {
                self.edit_playlist(&playlist, PlaylistEdit::Restore(removed), Notice::Undone);
            }
            Some(Undo::Relations(changes)) => self.relate(changes, None, Notice::Undone),
        }
    }
}
