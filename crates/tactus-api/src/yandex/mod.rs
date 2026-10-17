use crate::error::{Error, Result};
use crate::model::*;
use crate::net;
use crate::parse::{self, Json};
use crate::service::*;
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Method, RequestBuilder};
use serde_json::{Value, json};
use std::future::Future;
use yandex_music::api::account::consume_promo_code::ConsumePromoCodeOptions;
use yandex_music::api::album::add_liked_album::AddLikedAlbumOptions;
use yandex_music::api::album::remove_liked_album::RemoveLikedAlbumOptions;
use yandex_music::api::artist::add_disliked_artist::AddDislikedArtistOptions;
use yandex_music::api::artist::add_liked_artist::AddLikedArtistOptions;
use yandex_music::api::artist::remove_disliked_artist::RemoveDislikedArtistOptions;
use yandex_music::api::artist::remove_liked_artist::RemoveLikedArtistOptions;
use yandex_music::api::playlist::add_liked_playlist::AddLikedPlaylistOptions;
use yandex_music::api::playlist::remove_liked_playlist::RemoveLikedPlaylistOptions;
use yandex_music::api::track::add_disliked_tracks::AddDislikedTracksOptions;
use yandex_music::api::track::add_liked_tracks::AddLikedTracksOptions;
use yandex_music::api::track::get_download_info::GetDownloadInfoOptions;
use yandex_music::api::track::get_file_info::{Codec as FileCodec, GetFileInfoOptions};
use yandex_music::api::track::get_lyrics::GetLyricsOptions;
use yandex_music::api::track::play_audio::PlayAudioOptions;
use yandex_music::api::track::remove_disliked_tracks::RemoveDislikedTracksOptions;
use yandex_music::api::track::remove_liked_tracks::RemoveLikedTracksOptions;
use yandex_music::model::info::file_info::Quality as FileQuality;
use yandex_music::model::info::lyrics::LyricsFormat;
use yandex_music::{DEFAULT_CLIENT_ID, YandexMusicClient};

mod account;
mod catalog;
mod library;
#[cfg(test)]
mod live;
mod playback;

const API_URL: &str = "https://api.music.yandex.net";
const DEVICE: &str =
    "os=Windows; os_version=10; manufacturer=Tactus; model=Tactus; clid=; device_id=tactus; uuid=tactus";
const TRACK_CHUNK: usize = 200;

pub struct YandexService {
    http: reqwest::Client,
    client: YandexMusicClient,
}

async fn send(request: RequestBuilder) -> Result<Value> {
    let response = request.send().await?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or_default();
    if status.is_success() {
        return Ok(value);
    }
    let error = value.at("error");
    let message = [error.text("message"), error.text("name"), value.text("error")]
        .into_iter()
        .find(|text| !text.is_empty())
        .map_or_else(|| status.to_string(), str::to_owned);
    Err(Error::Status { status: status.as_u16(), message })
}

fn url(path: &str) -> String {
    format!("{API_URL}/{path}")
}

fn track_ref(track: &Track) -> Value {
    json!({ "id": track.id, "albumId": track.album_id })
}

impl YandexService {
    pub fn new(token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("X-Yandex-Music-Client", HeaderValue::from_static(DEFAULT_CLIENT_ID));
        if !token.is_empty() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("OAuth {token}")).map_err(|error| Error::Api(error.to_string()))?,
            );
        }
        let http = net::builder(headers).build()?;
        let client = YandexMusicClient::builder(token).custom_client(http.clone()).build()?;
        Ok(Self { http, client })
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> impl Future<Output = Result<Value>> + use<> {
        send(self.http.get(url(path)).query(query))
    }

    fn post(&self, path: &str, form: &[(&str, &str)]) -> impl Future<Output = Result<Value>> + use<> {
        send(self.http.post(url(path)).form(form))
    }

    fn post_json(&self, path: &str, body: &Value) -> impl Future<Output = Result<Value>> + use<> {
        self.send_json(Method::POST, path, body)
    }

    fn send_json(&self, method: Method, path: &str, body: &Value) -> impl Future<Output = Result<Value>> + use<> {
        send(self.http.request(method, url(path)).header("X-Yandex-Music-Device", DEVICE).json(body))
    }

    async fn fetch_tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        let mut tracks = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(TRACK_CHUNK) {
            let ids = chunk.join(",");
            let value = self.post("tracks", &[("track-ids", ids.as_str()), ("with-positions", "false")]).await?;
            tracks.extend(parse::tracks(parse::result(&value).as_array().map(Vec::as_slice).unwrap_or_default()));
        }
        Ok(tracks)
    }

    async fn fetch_playlists(&self, keys: &[PlaylistKey]) -> Result<Vec<Playlist>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = keys.iter().map(|key| format!("{}:{}", key.owner, key.kind)).collect();
        let ids = ids.join(",");
        Ok(parse::playlists(&self.post("playlists/list", &[("playlist-ids", ids.as_str())]).await?))
    }
}
