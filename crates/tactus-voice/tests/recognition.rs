use std::path::{Path, PathBuf};
use tactus_voice::{Intent, Language, Model, NameKind, Phrases, RATE, Recognizer, SleepRequest, Vosk};

fn folder() -> PathBuf {
    std::env::var("TACTUS_VOICE_TEST")
        .map_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/voice-test"), PathBuf::from)
}

fn samples(path: &Path) -> Vec<i16> {
    let bytes = std::fs::read(path).unwrap();
    let data = bytes.windows(4).position(|window| window == b"data").unwrap() + 8;
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), RATE, "{}", path.display());
    bytes[data..].as_chunks::<2>().0.iter().map(|pair| i16::from_le_bytes(*pair)).collect()
}

fn hear(recognizer: &mut Recognizer, path: &Path) -> String {
    for chunk in samples(path).chunks(4_000) {
        recognizer.accept(chunk);
    }
    recognizer.finish()
}

#[test]
#[ignore]
fn recognizes_synthesized_commands() {
    let folder = folder();
    let vosk = Vosk::load(&folder.join("vosk-win64-0.3.45")).unwrap();
    let cases: [(Language, &str, &str, &str, Option<Intent>); 12] = [
        (Language::Russian, "vosk-model-small-ru-0.22", "next", "тактус", Some(Intent::Next)),
        (Language::Russian, "vosk-model-small-ru-0.22", "volume", "тактус", Some(Intent::Volume(30))),
        (
            Language::Russian,
            "vosk-model-small-ru-0.22",
            "album",
            "тактус",
            Some(Intent::PlayName { kind: NameKind::Album, name: "кино".into() }),
        ),
        (Language::Russian, "vosk-model-small-ru-0.22", "pause", "тактус", Some(Intent::Pause)),
        (Language::Russian, "vosk-model-small-ru-0.22", "wave", "тактус", Some(Intent::PlayWave)),
        (
            Language::Russian,
            "vosk-model-small-ru-0.22",
            "sleep",
            "тактус",
            Some(Intent::Sleep(SleepRequest::Minutes(20))),
        ),
        (Language::English, "vosk-model-small-en-us-0.15", "next", "tactus", Some(Intent::Next)),
        (Language::English, "vosk-model-small-en-us-0.15", "volume", "tactus", Some(Intent::Volume(40))),
        (
            Language::English,
            "vosk-model-small-en-us-0.15",
            "artist",
            "tactus",
            Some(Intent::PlayName { kind: NameKind::Artist, name: "queen".into() }),
        ),
        (Language::English, "vosk-model-small-en-us-0.15", "pause", "tactus", Some(Intent::Pause)),
        (Language::English, "vosk-model-small-en-us-0.15", "wake", "tactus", None),
        (Language::Russian, "vosk-model-small-ru-0.22", "wake", "тактус", None),
    ];
    let mut failures = Vec::new();
    let mut loaded: Option<(&str, std::sync::Arc<Model>)> = None;
    for (language, model_name, sample, wake, expected) in cases {
        if loaded.as_ref().is_none_or(|(name, _)| *name != model_name) {
            loaded = Some((model_name, Model::load(&vosk, &folder.join(model_name)).unwrap()));
        }
        let model = loaded.as_ref().map(|(_, model)| model.clone()).unwrap();
        let phrases = Phrases::builtin(language);
        let code = language.code();
        eprintln!("{code}: model knows wake word {wake}: {}", model.knows(wake));
        for voice in 1..=2 {
            let path = folder.join(format!("{code}-{sample}-{voice}.wav"));
            let mut recognizer = Recognizer::new(&model, RATE, None).unwrap();
            let heard = hear(&mut recognizer, &path);
            let intent = phrases.interpret(&heard);
            eprintln!("{code}-{sample}-{voice}: heard {heard:?} -> {intent:?}");
            if expected.is_some() && intent != expected {
                failures.push(format!("{code}-{sample}-{voice}: heard {heard:?}, got {intent:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
