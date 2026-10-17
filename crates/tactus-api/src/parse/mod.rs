use crate::model::*;
use serde_json::Value;
use std::collections::HashSet;

mod account;
mod catalog;
mod entity;
mod feed;
mod genre;
mod playback;
mod search;

pub use account::*;
pub use catalog::*;
pub use entity::*;
pub use feed::*;
pub use genre::*;
pub use playback::*;
pub use search::*;

static NULL: Value = Value::Null;

pub trait Json {
    fn at(&self, key: &str) -> &Value;
    fn text(&self, key: &str) -> &str;
    fn id(&self, key: &str) -> String;
    fn num(&self, key: &str) -> u64;
    fn flag(&self, key: &str) -> bool;
    fn list(&self, key: &str) -> &[Value];
    fn unwrap(&self, key: &str) -> &Value;
}

impl Json for Value {
    fn at(&self, key: &str) -> &Value {
        if key.starts_with('/') { self.pointer(key).unwrap_or(&NULL) } else { self.get(key).unwrap_or(&NULL) }
    }

    fn text(&self, key: &str) -> &str {
        self.at(key).as_str().unwrap_or_default()
    }

    fn id(&self, key: &str) -> String {
        match self.at(key) {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => String::new(),
        }
    }

    fn num(&self, key: &str) -> u64 {
        match self.at(key) {
            Value::Number(number) => number.as_u64().unwrap_or_default(),
            Value::String(text) => text.parse().unwrap_or_default(),
            _ => 0,
        }
    }

    fn flag(&self, key: &str) -> bool {
        match self.at(key) {
            Value::Bool(value) => *value,
            Value::String(text) => text == "true",
            _ => false,
        }
    }

    fn list(&self, key: &str) -> &[Value] {
        self.at(key).as_array().map(Vec::as_slice).unwrap_or_default()
    }

    fn unwrap(&self, key: &str) -> &Value {
        self.get(key).filter(|inner| inner.is_object()).unwrap_or(self)
    }
}

pub fn result(value: &Value) -> &Value {
    value.get("result").unwrap_or(value)
}

pub fn cover(uri: &str) -> String {
    if uri.is_empty() {
        return String::new();
    }
    let uri = uri.replace("%%", "400x400");
    let uri = uri.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("//");
    format!("https://{uri}")
}

fn names(artists: &[Value]) -> String {
    artists.iter().map(|artist| artist.text("name")).filter(|name| !name.is_empty()).collect::<Vec<_>>().join(", ")
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty() && value != "0").then_some(value)
}

pub fn collect<T>(values: &[Value], parse: fn(&Value) -> Option<T>) -> Vec<T> {
    values.iter().filter_map(parse).collect()
}

fn item_of(kind: &str, value: &Value) -> Option<Item> {
    match kind {
        "track" | "podcast_episode" => track(value).map(Item::Track),
        "album" | "podcast" => album(value).map(Item::Album),
        "artist" => artist(value).map(Item::Artist),
        "playlist" => playlist(value).map(Item::Playlist),
        _ => None,
    }
}

fn items<T>(values: &[Value], parse: fn(&Value) -> Option<T>, wrap: fn(T) -> Item) -> Vec<Item> {
    values.iter().filter_map(parse).map(wrap).collect()
}

fn track_items(values: &[Value]) -> Vec<Item> {
    tracks(values).into_iter().map(Item::Track).collect()
}

fn unique(items: Vec<Item>) -> Vec<Item> {
    let mut unique: Vec<Item> = Vec::with_capacity(items.len());
    for item in items {
        if !unique.contains(&item) {
            unique.push(item);
        }
    }
    unique
}

fn push(sections: &mut Vec<Section>, kind: SectionKind, title: &str, items: Vec<Item>) {
    push_paged(sections, kind, title, items, None);
}

fn push_paged(sections: &mut Vec<Section>, kind: SectionKind, title: &str, items: Vec<Item>, more: Option<u32>) {
    if !items.is_empty() {
        sections.push(Section::new(kind, title, items).with_more(more));
    }
}

fn next_page(page: u32, per_page: u64, total: u64, shown: usize) -> Option<u32> {
    (shown > 0 && (u64::from(page) + 1) * per_page.max(1) < total).then_some(page + 1)
}

#[cfg(test)]
use testing::*;

#[cfg(test)]
mod testing {
    use super::*;

    pub fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    pub fn section(page: &Page, kind: SectionKind) -> Option<&Section> {
        page.sections.iter().find(|section| section.kind == kind)
    }

    pub fn kinds(page: &Page) -> Vec<SectionKind> {
        page.sections.iter().map(|section| section.kind).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_cover_urls() {
        assert_eq!(cover("avatars.yandex.net/get/1/%%"), "https://avatars.yandex.net/get/1/400x400");
        assert_eq!(cover(""), "");
    }
}
