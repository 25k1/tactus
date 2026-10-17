#![deny(unsafe_code)]

mod audio;
#[allow(unsafe_code)]
mod engine;
mod intent;
mod listener;
mod microphone;
mod names;
mod numbers;
mod pattern;
mod phrases;
mod text;

pub use audio::RATE;
pub use engine::{Model, Recognizer, Vosk};
pub use intent::{Intent, Mood, NameKind, Place, SleepRequest};
pub use listener::{Config, Control, Heard, Listener, Opener};
pub use microphone::Source;
pub use names::{Candidate, Pick, best_matches, pick, split_wake};
pub use numbers::parse_number;
pub use phrases::{Language, PhraseError, Phrases};
pub use text::normalize;
