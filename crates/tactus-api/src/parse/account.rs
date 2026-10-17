use super::*;

pub fn account(value: &Value) -> Account {
    let value = result(value);
    let account = value.at("account");
    let login = account.text("login").to_owned();
    let name = [account.text("displayName"), account.text("fullName"), &login]
        .into_iter()
        .find(|name| !name.is_empty())
        .unwrap_or_default()
        .to_owned();
    Account { uid: account.num("uid"), login, name, has_plus: value.at("plus").flag("hasPlus") }
}

pub fn account_settings(value: &Value) -> AccountSettings {
    let value = result(value);
    AccountOption::ALL
        .into_iter()
        .filter_map(|option| {
            let setting = value.get(option.key())?;
            Some((option, setting.as_bool().unwrap_or_else(|| setting.as_str() == Some("PUBLIC"))))
        })
        .collect()
}

pub fn library_track_ids(value: &Value) -> Vec<String> {
    result(value).at("library").list("tracks").iter().map(|track| track.id("id")).filter(|id| !id.is_empty()).collect()
}

pub fn collection(
    liked: Vec<Track>,
    albums: &Value,
    artists: &Value,
    playlists_value: &Value,
    disliked: Vec<Track>,
    disliked_artists: &Value,
) -> Page {
    let list = |value: &Value| result(value).as_array().cloned().unwrap_or_default();
    let (podcasts, albums): (Vec<Album>, Vec<Album>) = collect(&list(albums), album)
        .into_iter()
        .partition(|album| matches!(album.kind, AlbumKind::Podcast | AlbumKind::Audiobook));
    let mut sections = Vec::new();
    push(&mut sections, SectionKind::Tracks, "", liked.into_iter().map(Item::Track).collect());
    push(&mut sections, SectionKind::Albums, "", albums.into_iter().map(Item::Album).collect());
    push(&mut sections, SectionKind::Artists, "", items(&list(artists), artist, Item::Artist));
    push(&mut sections, SectionKind::Playlists, "", items(&list(playlists_value), playlist, Item::Playlist));
    push(&mut sections, SectionKind::Podcasts, "", podcasts.into_iter().map(Item::Album).collect());
    push(&mut sections, SectionKind::DislikedTracks, "", disliked.into_iter().map(Item::Track).collect());
    push(&mut sections, SectionKind::DislikedArtists, "", items(&list(disliked_artists), artist, Item::Artist));
    Page { sections, ..Page::default() }
}

pub fn pins(value: &Value) -> Vec<Item> {
    result(value)
        .list("pins")
        .iter()
        .filter_map(|pin| {
            let data = pin.at("data");
            match pin.text("type") {
                "album_item" => album(data).map(Item::Album),
                "artist_item" => artist(data).map(Item::Artist),
                "playlist_item" => playlist(data).map(Item::Playlist),
                _ => None,
            }
        })
        .collect()
}

pub fn liked_keys<T>(value: &Value, parse: fn(&Value) -> Option<T>) -> Vec<T> {
    collect(result(value).as_array().map(Vec::as_slice).unwrap_or_default(), parse)
}

pub fn device_code(value: &Value) -> DeviceCode {
    DeviceCode {
        device_code: value.text("device_code").to_owned(),
        user_code: value.text("user_code").to_owned(),
        verification_url: value.text("verification_url").to_owned(),
        expires_in: value.num("expires_in"),
        interval: value.num("interval"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_account() {
        let account = account(&fixture("account_status"));
        assert_eq!(account.uid, 1000);
        assert_eq!(account.name, "Listener");
        assert!(account.has_plus);
    }

    #[test]
    fn parses_account_settings() {
        let settings = account_settings(&fixture("account_settings"));
        assert_eq!(settings.len(), AccountOption::ALL.len());
        assert!(settings.contains(&(AccountOption::PublicCollection, true)));
    }

    #[test]
    fn parses_pins() {
        let pins = pins(&fixture("pins"));
        assert_eq!(pins.len(), 3);
        assert!(matches!(&pins[0], Item::Album(album) if album.id == "39611356"));
        assert!(matches!(&pins[1], Item::Artist(artist) if artist.name == "Metallica"));
        assert!(
            matches!(&pins[2], Item::Playlist(playlist) if playlist.key == PlaylistKey { owner: 103372440, kind: 2131 })
        );
    }

    #[test]
    fn parses_collection() {
        let liked = library_track_ids(&fixture("likes_tracks"));
        assert_eq!(liked.len(), 2);
        let page = collection(
            Vec::new(),
            &fixture("likes_albums"),
            &fixture("likes_artists"),
            &fixture("likes_playlists"),
            Vec::new(),
            &Value::Null,
        );
        assert_eq!(kinds(&page), [SectionKind::Albums, SectionKind::Artists, SectionKind::Playlists]);
        assert_eq!(liked_keys(&fixture("likes_playlists"), liked_playlist_key).len(), 2);
    }
}
