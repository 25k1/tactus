use std::sync::LazyLock;
use std::sync::mpsc::{Sender, channel};
use tactus_speech::Speech;

static VOICE: LazyLock<Option<Sender<String>>> = LazyLock::new(|| {
    let (sender, receiver) = channel::<String>();
    std::thread::Builder::new()
        .name("yandex-speech".into())
        .spawn(move || {
            let speech = Speech::new();
            for text in receiver {
                speech.say(&text, true);
            }
        })
        .ok()
        .map(|_| sender)
});

pub fn say(text: impl Into<String>) {
    if let Some(voice) = VOICE.as_ref() {
        let _ = voice.send(text.into());
    }
}
