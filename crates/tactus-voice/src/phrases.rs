use crate::intent::{self, Intent, Slot};
use crate::pattern::{self, Match, Node};
use crate::text::normalize;

const RUSSIAN: &str = include_str!("../phrases/ru.txt");
const ENGLISH: &str = include_str!("../phrases/en.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Russian,
    English,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::Russian, Language::English];

    pub fn code(self) -> &'static str {
        match self {
            Language::Russian => "ru",
            Language::English => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Language> {
        Self::ALL.into_iter().find(|language| language.code() == code)
    }

    fn builtin(self) -> &'static str {
        match self {
            Language::Russian => RUSSIAN,
            Language::English => ENGLISH,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhraseError {
    pub line: usize,
    pub message: String,
}

struct Rule {
    id: String,
    nodes: Vec<Node>,
    default: Option<u32>,
}

pub struct Phrases {
    language: Language,
    rules: Vec<Rule>,
}

fn contains(nodes: &[Node], wanted: &Node) -> bool {
    nodes.iter().any(|node| match node {
        Node::Choice(alternatives) | Node::Optional(alternatives) => {
            alternatives.iter().any(|alternative| contains(alternative, wanted))
        }
        node => node == wanted,
    })
}

fn render(nodes: &[Node], default: Option<u32>) -> String {
    let words: Vec<String> = nodes
        .iter()
        .filter_map(|node| match node {
            Node::Word(word) => Some(word.clone()),
            Node::Choice(alternatives) => alternatives.first().map(|first| render(first, default)),
            Node::Optional(_) => None,
            Node::Number => Some(default.map_or_else(|| "…".to_owned(), |number| number.to_string())),
            Node::Name => Some("…".to_owned()),
        })
        .collect();
    words.join(" ")
}

fn rule(id: &str, slot: Slot, alternative: &str) -> Result<Rule, String> {
    let (phrase, default) = match alternative.split_once("=>") {
        Some((phrase, default)) => {
            (phrase.trim(), Some(default.trim().parse::<u32>().map_err(|_| format!("bad number {default}"))?))
        }
        None => (alternative, None),
    };
    let nodes = pattern::parse(phrase)?;
    let (number, name) = (contains(&nodes, &Node::Number), contains(&nodes, &Node::Name));
    let fits = match slot {
        Slot::Nothing => !number && !name && default.is_none(),
        Slot::Number => (number || default.is_some()) && !name,
        Slot::Name => name && !number,
    };
    if !fits {
        return Err(format!("\"{phrase}\" does not fit {id}"));
    }
    Ok(Rule { id: id.to_owned(), nodes, default })
}

impl Phrases {
    pub fn builtin(language: Language) -> Phrases {
        let mut phrases = Phrases { language, rules: Vec::new() };
        phrases.extend(language.builtin());
        phrases
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn extend(&mut self, text: &str) -> Vec<PhraseError> {
        let mut errors = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let error = |message: String| PhraseError { line: index + 1, message };
            let Some((id, body)) = line.split_once('=') else {
                errors.push(error("expected: command = phrase | phrase".into()));
                continue;
            };
            let id = id.trim();
            let Some(slot) = intent::slot(id) else {
                errors.push(error(format!("unknown command {id}")));
                continue;
            };
            for alternative in pattern::split_alternatives(body) {
                match rule(id, slot, &alternative) {
                    Ok(rule) => self.rules.push(rule),
                    Err(message) => errors.push(error(message)),
                }
            }
        }
        errors
    }

    pub fn interpret(&self, text: &str) -> Option<Intent> {
        let text = normalize(text);
        let tokens: Vec<&str> = text.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }
        [false, true].into_iter().find_map(|fuzzy| {
            let mut best: Option<(&Rule, Match)> = None;
            for rule in &self.rules {
                if let Some(found) = pattern::matches(&rule.nodes, &tokens, fuzzy)
                    && best.as_ref().is_none_or(|(_, current)| found.better_than(current))
                {
                    best = Some((rule, found));
                }
            }
            let (rule, found) = best?;
            intent::build(&rule.id, found.captures.number.or(rule.default), found.captures.name)
        })
    }

    pub fn examples(&self) -> Vec<String> {
        let mut examples: Vec<(&str, String)> = Vec::new();
        for rule in &self.rules {
            if !examples.iter().any(|(id, _)| *id == rule.id) {
                examples.push((&rule.id, render(&rule.nodes, rule.default)));
            }
        }
        examples.into_iter().map(|(_, example)| example).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{Mood, NameKind, Place, SleepRequest};

    fn named(kind: NameKind, name: &str) -> Intent {
        Intent::PlayName { kind, name: name.into() }
    }

    fn check(language: Language, cases: &[(&str, Option<Intent>)]) {
        let phrases = Phrases::builtin(language);
        for (heard, expected) in cases {
            assert_eq!(&phrases.interpret(heard), expected, "{heard}");
        }
    }

    #[test]
    fn builtin_phrases_are_valid_and_cover_every_command() {
        for language in Language::ALL {
            let mut phrases = Phrases { language, rules: Vec::new() };
            assert_eq!(phrases.extend(language.builtin()), [], "{language:?}");
            for (id, _) in intent::IDS {
                assert!(phrases.rules.iter().any(|rule| rule.id == id), "{language:?} has no phrase for {id}");
            }
        }
    }

    #[test]
    fn understands_russian_commands() {
        check(
            Language::Russian,
            &[
                ("Пауза", Some(Intent::Pause)),
                ("включи", Some(Intent::Play)),
                ("следующая песня", Some(Intent::Next)),
                ("включи следующий трек", Some(Intent::Next)),
                ("сделай громкость на тридцать пять процентов", Some(Intent::Volume(35))),
                ("громкость сто пятьдесят", None),
                ("перемотай вперед на двадцать секунд", Some(Intent::Forward(20))),
                ("отмотай назад", Some(Intent::Rewind(10))),
                ("выключись через полчаса", Some(Intent::Sleep(SleepRequest::Minutes(30)))),
                ("усни через час", Some(Intent::Sleep(SleepRequest::Minutes(60)))),
                ("выключи музыку после этой песни", Some(Intent::Sleep(SleepRequest::EndOfTrack))),
                ("включи альбом Master of Puppets", Some(named(NameKind::Album, "master of puppets"))),
                ("поставь группу Кино", Some(named(NameKind::Artist, "кино"))),
                ("включи Металлику", Some(named(NameKind::Any, "металлику"))),
                ("включи мою волну", Some(Intent::PlayWave)),
                ("включи что-нибудь спокойное", Some(Intent::PlayMood(Mood::Calm))),
                ("включи любимые треки", Some(Intent::PlayLikes)),
                ("добавь в плейлист кавказ", Some(Intent::AddToPlaylist("кавказ".into()))),
                ("найди Битлз", Some(Intent::Search("битлз".into()))),
                ("открой очередь", Some(Intent::Open(Place::Queue))),
                ("что сейчас играет", Some(Intent::WhatsPlaying)),
                ("убери лайк", Some(Intent::Unlike)),
                ("что можно сказать", Some(Intent::Help)),
                ("следущий", Some(Intent::Next)),
                ("какая-то бессмыслица про погоду", None),
            ],
        );
    }

    #[test]
    fn understands_english_commands() {
        check(
            Language::English,
            &[
                ("play", Some(Intent::Play)),
                ("Stop!", Some(Intent::Pause)),
                ("skip this song", Some(Intent::Next)),
                ("set the volume to forty", Some(Intent::Volume(40))),
                ("turn it up", Some(Intent::VolumeUp)),
                ("go back thirty seconds", Some(Intent::Rewind(30))),
                ("stop in half an hour", Some(Intent::Sleep(SleepRequest::Minutes(30)))),
                ("play the album Abbey Road", Some(named(NameKind::Album, "abbey road"))),
                ("play music by Queen", Some(named(NameKind::Artist, "queen"))),
                ("play Metallica", Some(named(NameKind::Any, "metallica"))),
                ("play something relaxing", Some(Intent::PlayMood(Mood::Calm))),
                ("play my wave", Some(Intent::PlayWave)),
                ("add this to my playlist road trip", Some(Intent::AddToPlaylist("road trip".into()))),
                ("what's playing", Some(Intent::WhatsPlaying)),
                ("go back", Some(Intent::GoBack)),
                ("never mind", Some(Intent::No)),
                ("tell me about the weather", None),
            ],
        );
    }

    #[test]
    fn user_phrases_extend_and_report_errors() {
        let mut phrases = Phrases::builtin(Language::Russian);
        let errors =
            phrases.extend("# my phrases\nnext = погнали дальше\nfly = улетай\nvolume = громкость\nplay = (включай\n");
        assert_eq!(errors.iter().map(|error| error.line).collect::<Vec<_>>(), [3, 4, 5]);
        assert_eq!(phrases.interpret("погнали дальше"), Some(Intent::Next));
    }

    #[test]
    fn examples_show_one_phrase_per_command() {
        let examples = Phrases::builtin(Language::English).examples();
        assert_eq!(examples.len(), intent::IDS.len());
        assert!(
            examples.contains(&"volume …".to_owned()) || examples.iter().any(|example| example.starts_with("volume"))
        );
    }

    proptest::proptest! {
        #[test]
        fn interpreting_never_panics(text in "\\PC{0,60}") {
            for language in Language::ALL {
                let _ = Phrases::builtin(language).interpret(&text);
            }
        }
    }
}
