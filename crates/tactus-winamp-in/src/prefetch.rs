use crate::host::Host;
use crate::library::{self, Source};
use std::sync::Mutex;
use tactus_api::sync::Guard;
use tactus_api::{Quality, net};
use tactus_winamp::log;

const AHEAD_MS: i32 = 30_000;

struct Ready {
    id: String,
    quality: Quality,
    source: Source,
}

impl Ready {
    fn serves(&self, id: &str, quality: Quality) -> bool {
        self.id == id && self.quality == quality && !self.source.buffer.failed()
    }
}

struct State {
    generation: u64,
    armed: bool,
    ready: Option<Ready>,
}

static STATE: Mutex<State> = Mutex::new(State { generation: 0, armed: false, ready: None });

fn due(position_ms: i32, length_ms: i32) -> bool {
    length_ms > 0 && position_ms >= length_ms.saturating_sub(AHEAD_MS)
}

pub fn arm() {
    let mut state = STATE.guard();
    state.generation += 1;
    state.armed = true;
}

pub fn take(id: &str, quality: Quality) -> Option<Source> {
    let ready = STATE.guard().ready.take()?;
    ready.serves(id, quality).then_some(ready.source)
}

pub fn consider(position_ms: i32, length_ms: i32) {
    if !due(position_ms, length_ms) {
        return;
    }
    let generation = {
        let mut state = STATE.guard();
        if !std::mem::take(&mut state.armed) {
            return;
        }
        state.generation
    };
    let Some(id) = Host::get().winamp().following_track_id() else {
        return;
    };
    let quality = library::quality();
    net::spawn_blocking(move || match library::source(&id, quality) {
        Ok(source) => {
            let mut state = STATE.guard();
            if state.generation == generation {
                state.ready = Some(Ready { id, quality, source });
            }
        }
        Err(error) => log::write(format_args!("preparing {id}: {error}")),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tactus_api::Codec;
    use tactus_stream::Buffer;

    #[test]
    fn prepares_during_the_last_half_minute_of_known_lengths() {
        assert!(!due(0, 200_000));
        assert!(!due(169_999, 200_000));
        assert!(due(170_000, 200_000));
        assert!(due(5_000, 20_000));
        assert!(!due(500_000, -1));
        assert!(!due(500_000, 0));
    }

    #[test]
    fn serves_only_the_same_track_and_quality() {
        let ready = Ready {
            id: "42".into(),
            quality: Quality::High,
            source: Source { buffer: Buffer::complete(vec![1, 2, 3]), codec: Codec::Mp3 },
        };
        assert!(ready.serves("42", Quality::High));
        assert!(!ready.serves("43", Quality::High));
        assert!(!ready.serves("42", Quality::Lossless));
    }
}
