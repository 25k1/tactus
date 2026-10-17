use crate::text::{distance, normalize};

const ACCEPT: f32 = 0.6;
const CONFIDENT: f32 = 0.8;
const MARGIN: f32 = 0.06;

const CYRILLIC: [(char, &str); 32] = [
    ('а', "a"),
    ('б', "b"),
    ('в', "v"),
    ('г', "g"),
    ('д', "d"),
    ('е', "e"),
    ('ж', "zh"),
    ('з', "z"),
    ('и', "i"),
    ('й', "i"),
    ('к', "k"),
    ('л', "l"),
    ('м', "m"),
    ('н', "n"),
    ('о', "o"),
    ('п', "p"),
    ('р', "r"),
    ('с', "s"),
    ('т', "t"),
    ('у', "u"),
    ('ф', "f"),
    ('х', "h"),
    ('ц', "ts"),
    ('ч', "ch"),
    ('ш', "sh"),
    ('щ', "sch"),
    ('ъ', ""),
    ('ы', "i"),
    ('ь', ""),
    ('э', "e"),
    ('ю', "yu"),
    ('я', "ya"),
];

const SKELETON: usize = 4;
const SKELETON_WEIGHT: f32 = 0.95;

const FOLDS: [(&str, &str); 16] = [
    ("ph", "f"),
    ("ck", "k"),
    ("th", "t"),
    ("ee", "i"),
    ("ea", "i"),
    ("oo", "u"),
    ("ou", "u"),
    ("dzh", "j"),
    ("qu", "kv"),
    ("c", "k"),
    ("kh", "k"),
    ("q", "k"),
    ("x", "ks"),
    ("w", "v"),
    ("y", "i"),
    ("z", "s"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub index: usize,
    pub score: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    One(usize),
    Several(Vec<usize>),
    Nothing,
}

fn word_key(word: &str) -> String {
    let latin: String = word
        .chars()
        .map(|c| {
            CYRILLIC
                .iter()
                .find(|(cyrillic, _)| *cyrillic == c)
                .map_or_else(|| c.to_string(), |(_, latin)| (*latin).to_owned())
        })
        .collect();
    let folded = FOLDS.iter().fold(latin, |text, (from, to)| text.replace(from, to));
    let mut key = String::with_capacity(folded.len());
    for c in folded.chars() {
        if !key.ends_with(c) {
            key.push(c);
        }
    }
    key
}

fn key(text: &str) -> Vec<String> {
    let normalized = normalize(text);
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let words = match words.as_slice() {
        ["the" | "зе", rest @ ..] if !rest.is_empty() => rest,
        words => words,
    };
    words.iter().map(|word| word_key(word)).collect()
}

fn similarity(left: &str, right: &str) -> f32 {
    let longest = left.chars().count().max(right.chars().count());
    if longest == 0 {
        return 0.0;
    }
    1.0 - distance(left, right) as f32 / longest as f32
}

fn skeleton(text: &str) -> String {
    text.chars().filter(|c| !"aeiou".contains(*c)).collect()
}

fn score(query: &[String], name: &[String]) -> f32 {
    let (query_text, name_text) = (query.concat(), name.concat());
    let (query_bones, name_bones) = (skeleton(&query_text), skeleton(&name_text));
    let bones = if query_bones.chars().count() >= SKELETON && name_bones.chars().count() >= SKELETON {
        similarity(&query_bones, &name_bones) * SKELETON_WEIGHT
    } else {
        0.0
    };
    let whole = similarity(&query_text, &name_text).max(bones);
    let covered = !query.is_empty()
        && query.iter().all(|word| name.iter().any(|candidate| similarity(word, candidate) >= CONFIDENT));
    if covered && query.len() < name.len() { whole.max(CONFIDENT + 0.01) } else { whole }
}

pub fn best_matches(query: &str, names: &[&str]) -> Vec<Candidate> {
    let query = key(query);
    let mut candidates: Vec<Candidate> = names
        .iter()
        .enumerate()
        .map(|(index, name)| Candidate { index, score: score(&query, &key(name)) })
        .filter(|candidate| candidate.score >= ACCEPT)
        .collect();
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.index.cmp(&b.index)));
    candidates
}

pub fn pick(candidates: &[Candidate]) -> Pick {
    let Some(top) = candidates.first() else {
        return Pick::Nothing;
    };
    let close: Vec<usize> = candidates
        .iter()
        .filter(|candidate| top.score - candidate.score < MARGIN)
        .map(|candidate| candidate.index)
        .collect();
    match close.as_slice() {
        [only] if top.score >= CONFIDENT => Pick::One(*only),
        _ => Pick::Several(candidates.iter().map(|candidate| candidate.index).collect()),
    }
}

const WAKE_SPAN: usize = 3;
const WAKE_MATCH: f32 = 0.8;

pub fn split_wake<'a>(heard: &'a str, wake: &str) -> Option<Vec<&'a str>> {
    let wake = key(wake).concat();
    let words: Vec<&str> = heard.split_whitespace().collect();
    (0..words.len()).find_map(|start| {
        (1..=WAKE_SPAN.min(words.len() - start)).find_map(|span| {
            let spoken = key(&words[start..start + span].join(" ")).concat();
            (similarity(&spoken, &wake) >= WAKE_MATCH && skeleton(&spoken) == skeleton(&wake))
                .then(|| words[start + span..].to_vec())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY: [&str; 8] =
        ["Metallica", "The Beatles", "Кино", "Би-2", "Madonna", "Michael Jackson", "Pink Floyd", "Pink"];

    fn best(query: &str) -> Pick {
        pick(&best_matches(query, &LIBRARY))
    }

    #[test]
    fn matches_names_across_scripts_and_case_endings() {
        assert_eq!(best("металлику"), Pick::One(0));
        assert_eq!(best("битлз"), Pick::One(1));
        assert_eq!(best("кино"), Pick::One(2));
        assert_eq!(best("би два"), Pick::Nothing);
        assert_eq!(best("би 2"), Pick::One(3));
        assert_eq!(best("мадонну"), Pick::One(4));
        assert_eq!(best("майкл джексон"), Pick::One(5));
        assert_eq!(best("пинк флойд"), Pick::One(6));
    }

    #[test]
    fn exact_short_names_win_over_longer_ones() {
        assert_eq!(best("пинк"), Pick::One(7));
        assert_eq!(pick(&best_matches("квин", &["Queens of the Stone Age", "Queen"])), Pick::One(1));
    }

    #[test]
    fn ambiguous_or_unknown_names_are_not_guessed() {
        let close = [
            Candidate { index: 3, score: 0.9 },
            Candidate { index: 1, score: 0.87 },
            Candidate { index: 2, score: 0.7 },
        ];
        assert_eq!(pick(&close), Pick::Several(vec![3, 1, 2]));
        assert_eq!(pick(&[Candidate { index: 0, score: 0.7 }]), Pick::Several(vec![0]));
        assert_eq!(best("rammstein"), Pick::Nothing);
        assert_eq!(pick(&[]), Pick::Nothing);
    }

    #[test]
    fn spots_the_wake_word_as_the_models_hear_it() {
        let wake = |heard: &str, word: &str| split_wake(heard, word).map(|rest| rest.join(" "));
        assert_eq!(wake("так туз", "тактус").as_deref(), Some(""));
        assert_eq!(wake("так то с", "тактус").as_deref(), Some(""));
        assert_eq!(wake("tact is", "tactus").as_deref(), Some(""));
        assert_eq!(wake("так туз следующий трек", "тактус").as_deref(), Some("следующий трек"));
        assert_eq!(wake("ну так туз пауза", "тактус").as_deref(), Some("пауза"));
        for ordinary in ["так что", "так и быть", "тихо", "это так", "tactical response", "take us"]
        {
            assert_eq!(wake(ordinary, if ordinary.is_ascii() { "tactus" } else { "тактус" }), None, "{ordinary}");
        }
    }

    proptest::proptest! {
        #[test]
        fn a_name_always_finds_itself_first(index in 0usize..LIBRARY.len()) {
            let found = best_matches(LIBRARY[index], &LIBRARY);
            proptest::prop_assert_eq!(found.first().map(|candidate| candidate.index), Some(index));
        }
    }
}
