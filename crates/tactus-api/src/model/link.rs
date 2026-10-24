use super::*;

const SCHEME: &str = "yandexmusic://";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaylistLink {
    Owner { owner: String, kind: u32 },
    Shared(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    View(View),
    Playlist(PlaylistLink),
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

impl Destination {
    pub fn parse(link: &str) -> Option<Destination> {
        let path = link_path(link.trim())?;
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        let view = match parts.as_slice() {
            ["album", _, "track", id, ..] | ["track", id, ..] => View::Track((*id).to_owned()),
            ["album", id, ..] => View::Album((*id).to_owned()),
            ["artist", id, ..] => View::Artist((*id).to_owned()),
            ["users", owner, "playlists", kind, ..] => {
                let kind = kind.parse().ok()?;
                return Some(match owner.parse() {
                    Ok(owner) => Destination::View(View::Playlist(PlaylistKey { owner, kind })),
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
            ["non-music", "category", id, ..] => View::Category((*id).to_owned()),
            _ => return None,
        };
        Some(Destination::View(view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(link: &str) -> Option<View> {
        match Destination::parse(link)? {
            Destination::View(view) => Some(view),
            Destination::Playlist(_) => None,
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
        assert_eq!(
            view("https://music.yandex.ru/users/970829816/playlists/1104"),
            Some(View::Playlist(PlaylistKey { owner: 970829816, kind: 1104 }))
        );
    }

    #[test]
    fn keeps_playlists_that_need_a_lookup() {
        assert_eq!(
            Destination::parse("https://music.yandex.ru/users/yamusic-podcast/playlists/1104"),
            Some(Destination::Playlist(PlaylistLink::Owner { owner: "yamusic-podcast".into(), kind: 1104 }))
        );
        assert_eq!(
            Destination::parse("https://music.yandex.ru/playlists/lk.9f6f8c60-2440-7ad0-9a9f-7245afc086d2"),
            Some(Destination::Playlist(PlaylistLink::Shared("9f6f8c60-2440-7ad0-9a9f-7245afc086d2".into())))
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
            assert_eq!(Destination::parse(link), None, "{link}");
        }
    }
}
