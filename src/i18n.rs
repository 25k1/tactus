use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::unic_langid::LanguageIdentifier;
use i18n_embed::{DesktopLanguageRequester, LanguageLoader};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    let languages = loader.available_languages(&Localizations).unwrap_or_default();
    let _ = loader.load_languages(&Localizations, &languages);
    loader.set_use_isolating(false);
    loader
});

macro_rules! fl {
    ($($args:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $($args)*)
    };
}
pub(crate) use fl;

pub fn init(saved: Option<&str>) {
    let requested: Vec<LanguageIdentifier> = match saved.and_then(|code| code.parse().ok()) {
        Some(language) => vec![language],
        None => DesktopLanguageRequester::requested_languages(),
    };
    let _ = i18n_embed::select(&*LOADER, &Localizations, &requested);
    LOADER.set_use_isolating(false);
}

pub fn current_language() -> String {
    LOADER.current_languages().first().map_or_else(|| "en".to_owned(), |language| language.language.as_str().to_owned())
}

pub fn languages() -> Vec<(String, String)> {
    let mut languages = LOADER.available_languages(&Localizations).unwrap_or_default();
    languages.sort_by_key(|language| (language != LOADER.fallback_language(), language.to_string()));
    languages
        .into_iter()
        .map(|language| {
            let name = LOADER.select_languages(&[&language]).get("language-name");
            (language.to_string(), name)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashMap};

    const SOURCES: [(&str, &str); 3] = [
        ("en", include_str!("../i18n/en/tactus.ftl")),
        ("ka", include_str!("../i18n/ka/tactus.ftl")),
        ("ru", include_str!("../i18n/ru/tactus.ftl")),
    ];

    fn loader(code: &str) -> FluentLanguageLoader {
        LOADER.select_languages(&[code.parse::<LanguageIdentifier>().unwrap()])
    }

    fn ids(text: &str) -> BTreeSet<&str> {
        text.lines()
            .filter(|line| line.starts_with(|c: char| c.is_ascii_lowercase()))
            .filter_map(|line| line.split_once(" =").map(|(id, _)| id))
            .collect()
    }

    #[test]
    fn bundles_every_language() {
        let codes: Vec<String> = languages().into_iter().map(|(code, _)| code).collect();
        assert_eq!(codes, ["en", "ka", "ru"]);
    }

    #[test]
    fn plurals_follow_each_language() {
        let count = |code: &str, count: i64| loader(code).get_args("tracks-count", HashMap::from([("count", count)]));
        assert_eq!(
            [count("ru", 1), count("ru", 3), count("ru", 5), count("ru", 21)],
            ["1 трек", "3 трека", "5 треков", "21 трек"]
        );
        assert_eq!([count("ka", 1), count("ka", 5)], ["1 ტრეკი", "5 ტრეკი"]);
    }

    #[test]
    fn every_language_translates_every_message() {
        let english = ids(SOURCES[0].1);
        for (code, source) in SOURCES {
            let translated = ids(source);
            let missing: Vec<_> = english.difference(&translated).collect();
            let extra: Vec<_> = translated.difference(&english).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{code}: missing {missing:?}, extra {extra:?}");
            let loader = loader(code);
            let broken: Vec<_> = translated.iter().filter(|id| !loader.has(id)).collect();
            assert!(broken.is_empty(), "{code}: unparsed {broken:?}");
        }
        let georgian = |text: &str| text.chars().any(|c| ('\u{10A0}'..='\u{10FF}').contains(&c));
        let ka = loader("ka");
        let fallen_back: Vec<&str> = SOURCES[1]
            .1
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .filter(|(id, value)| {
                id.starts_with(|c: char| c.is_ascii_lowercase()) && georgian(value) && !georgian(&ka.get(id))
            })
            .map(|(id, _)| id)
            .collect();
        assert!(fallen_back.is_empty(), "ka falls back to English for {fallen_back:?}");
    }
}
