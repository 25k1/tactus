use tactus_api::{Destination, Item, PlaylistKey, PlaylistLink, View};

const SCHEME: &str = "yandexmusic://";
const WEB: &str = "https://music.yandex.ru";

fn query_value(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

fn link_path(link: &str) -> Option<&str> {
    if let Some(path) = link.strip_prefix(SCHEME) {
        return Some(path);
    }
    let rest = link.strip_prefix("https://").or_else(|| link.strip_prefix("http://")).unwrap_or(link);
    let (host, path) = rest.split_once('/')?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    let host = host.strip_prefix("next.").unwrap_or(host);
    host.strip_prefix("music.yandex.").filter(|domain| !domain.is_empty() && !domain.contains('.')).map(|_| path)
}

pub fn destination(link: &str) -> Option<Destination> {
    let full = link_path(link.trim())?;
    let path = full.split(['?', '#']).next().unwrap_or_default();
    let query = full.split_once('?').map(|(_, query)| query.split('#').next().unwrap_or_default()).unwrap_or_default();
    let destination = parse_path(path)?;
    match (destination, query_value(query, "token")) {
        (Destination::View(View::Playlist(key)), Some(token)) => Some(Destination::Join {
            playlist: PlaylistLink::Owner { kind: key.id.parse().unwrap_or_default(), owner: key.owner },
            token,
        }),
        (Destination::Playlist(playlist), Some(token)) => Some(Destination::Join { playlist, token }),
        (destination, _) => Some(destination),
    }
}

fn parse_path(path: &str) -> Option<Destination> {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let view = match parts.as_slice() {
        ["album", _, "track", id, ..] | ["track", id, ..] => View::Track((*id).to_owned()),
        ["album", id, ..] => View::Album((*id).to_owned()),
        ["artist", id, ..] => View::Artist((*id).to_owned()),
        ["users", owner, "playlists", kind, ..] => {
            let kind = kind.parse().ok()?;
            return Some(match owner.parse() {
                Ok::<u64, _>(owner) => Destination::View(View::Playlist(PlaylistKey::new(owner, kind))),
                Err(_) => Destination::Playlist(PlaylistLink::Owner { owner: (*owner).to_owned(), kind }),
            });
        }
        ["playlists" | "playlist", id, ..] => {
            let id = id.strip_prefix("lk.").unwrap_or(id);
            return Some(Destination::Playlist(PlaylistLink::Shared(id.to_owned())));
        }
        ["users", login, ..] => View::User((*login).to_owned()),
        ["tag", id, ..] => View::Tag((*id).to_owned()),
        ["chart", region, ..] => View::Chart((*region).to_owned()),
        ["label", id, ..] => View::Label((*id).to_owned()),
        ["non-music", "category", id, ..] => View::Category((*id).to_owned()),
        _ => return None,
    };
    Some(Destination::View(view))
}

pub fn share(item: &Item) -> Option<String> {
    Some(match item {
        Item::Track(track) if track.album_id.is_empty() => format!("{WEB}/track/{}", track.id),
        Item::Track(track) => format!("{WEB}/album/{}/track/{}", track.album_id, track.id),
        Item::Album(album) => format!("{WEB}/album/{}", album.id),
        Item::Artist(artist) => format!("{WEB}/artist/{}", artist.id),
        Item::Playlist(playlist) => format!("{WEB}/users/{}/playlists/{}", playlist.key.owner, playlist.key.id),
        Item::Tag(tag) => format!("{WEB}/tag/{}", tag.id),
        Item::Category(category) => format!("{WEB}/non-music/category/{}", category.id),
        Item::Chart(_) => format!("{WEB}/chart"),
        Item::Link(link) => link.url.clone(),
        Item::Concert(concert) => concert.url.clone(),
        Item::Station(_) | Item::Genre(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(link: &str) -> Option<View> {
        match destination(link)? {
            Destination::View(view) => Some(view),
            Destination::Playlist(_) | Destination::Join { .. } => None,
        }
    }

    #[test]
    fn opens_web_and_app_links() {
        assert_eq!(view("https://music.yandex.ru/album/123/track/456?utm=x"), Some(View::Track("456".into())));
        assert_eq!(view("music.yandex.com/track/7"), Some(View::Track("7".into())));
        assert_eq!(view(" https://next.music.yandex.ru/album/123 "), Some(View::Album("123".into())));
        assert_eq!(view("https://music.yandex.kz/artist/3121/tracks"), Some(View::Artist("3121".into())));
        assert_eq!(view("yandexmusic://album/9"), Some(View::Album("9".into())));
        assert_eq!(
            view("yandexmusic://non-music/category/AUDIOBOOKS_MENU"),
            Some(View::Category("AUDIOBOOKS_MENU".into()))
        );
        assert_eq!(view("https://music.yandex.ru/users/music-blog"), Some(View::User("music-blog".into())));
        assert_eq!(view("https://music.yandex.ru/tag/rock/"), Some(View::Tag("rock".into())));
        assert_eq!(view("https://music.yandex.ru/chart/world#top"), Some(View::Chart("world".into())));
        assert_eq!(view("https://music.yandex.ru/label/6074020"), Some(View::Label("6074020".into())));
        assert_eq!(
            view("https://music.yandex.ru/users/970829816/playlists/1104"),
            Some(View::Playlist(PlaylistKey::new(970829816, 1104)))
        );
    }

    #[test]
    fn keeps_playlists_that_need_a_lookup() {
        assert_eq!(
            destination("https://music.yandex.ru/users/yamusic-podcast/playlists/1104"),
            Some(Destination::Playlist(PlaylistLink::Owner { owner: "yamusic-podcast".into(), kind: 1104 }))
        );
        assert_eq!(
            destination("https://music.yandex.ru/playlists/lk.9f6f8c60-2440-7ad0-9a9f-7245afc086d2"),
            Some(Destination::Playlist(PlaylistLink::Shared("9f6f8c60-2440-7ad0-9a9f-7245afc086d2".into())))
        );
    }

    #[test]
    fn reads_collective_invites() {
        assert_eq!(
            destination("https://music.yandex.ru/users/970829816/playlists/1104?token=abc123&utm=x"),
            Some(Destination::Join {
                playlist: PlaylistLink::Owner { owner: "970829816".into(), kind: 1104 },
                token: "abc123".into()
            })
        );
        assert_eq!(
            destination("https://music.yandex.ru/users/someone/playlists/7?token=t"),
            Some(Destination::Join {
                playlist: PlaylistLink::Owner { owner: "someone".into(), kind: 7 },
                token: "t".into()
            })
        );
        assert_eq!(
            destination("https://music.yandex.ru/album/1?token=t"),
            Some(Destination::View(View::Album("1".into())))
        );
    }

    #[test]
    fn rejects_other_links() {
        for link in [
            "",
            "https://example.com/album/1",
            "https://music.yandex.ru/",
            "https://music.yandex.ru/users/a/playlists/x",
            "https://music.yandex.evil.com/album/1",
            "https://yandex.ru/album/1",
            "yandexmusic://unknown/1",
        ] {
            assert_eq!(destination(link), None, "{link}");
        }
    }

    #[test]
    fn shares_round_trip_through_the_parser() {
        let track = Item::Track(tactus_api::fake::track("456"));
        let link = share(&track).unwrap();
        assert_eq!(link, "https://music.yandex.ru/album/70/track/456");
        assert_eq!(destination(&link), Some(Destination::View(View::Track("456".into()))));
    }
}
