use super::*;

const BOUNDARY: &str = "tactus-upload-7f3a9c";

fn multipart(name: &str, data: Vec<u8>) -> Vec<u8> {
    let name = name.replace(['"', '\r', '\n'], "_");
    let head = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: audio/mpeg\r\n\r\n"
    );
    let tail = format!("\r\n--{BOUNDARY}--\r\n");
    [head.into_bytes(), data, tail.into_bytes()].concat()
}

fn playlist_diff(edit: &PlaylistEdit) -> Option<Value> {
    let delete = |index: usize, track: &Track| json!({ "op": "delete", "from": index, "to": index + 1, "tracks": [track_ref(track)] });
    let insert = |at: usize, tracks: Vec<Value>| json!({ "op": "insert", "at": at, "tracks": tracks });
    Some(Value::Array(match edit {
        PlaylistEdit::Insert { at, tracks } => vec![insert(*at as usize, tracks.iter().map(track_ref).collect())],
        PlaylistEdit::Remove(removed) => {
            let mut removed: Vec<&(usize, Track)> = removed.iter().collect();
            removed.sort_by_key(|(index, _)| std::cmp::Reverse(*index));
            removed.into_iter().map(|(index, track)| delete(*index, track)).collect()
        }
        PlaylistEdit::Restore(restored) => {
            let mut restored: Vec<&(usize, Track)> = restored.iter().collect();
            restored.sort_by_key(|(index, _)| *index);
            restored.into_iter().map(|(index, track)| insert(*index, vec![track_ref(track)])).collect()
        }
        PlaylistEdit::Move { from, to, track } => vec![delete(*from, track), insert(*to, vec![track_ref(track)])],
        _ => return None,
    }))
}

impl YandexService {
    async fn playlist_revision(&self, uid: u64, kind: u32) -> Result<u64> {
        let value = self.get(&format!("users/{uid}/playlists/{kind}"), &[]).await?;
        Ok(parse::result(&value).num("revision"))
    }
}

impl LibraryService for YandexService {
    fn library(&self, uid: u64) -> Result<Library> {
        let base = format!("users/{uid}");
        net::block_on(async {
            let (liked, disliked, albums, artists, playlists, disliked_artists, pins) = tokio::join!(
                self.get(&format!("{base}/likes/tracks"), &[]),
                self.get(&format!("{base}/dislikes/tracks"), &[]),
                self.get(&format!("{base}/likes/albums"), &[]),
                self.get(&format!("{base}/likes/artists"), &[("with-timestamps", "false")]),
                self.get(&format!("{base}/likes/playlists"), &[]),
                self.get(&format!("{base}/dislikes/artists"), &[]),
                self.get("pins", &[]),
            );
            let ids = |value: Result<Value>, parse: fn(&Value) -> Option<String>| {
                parse::liked_keys(&value.unwrap_or_default(), parse).into_iter().collect()
            };
            Ok::<_, Error>(Library {
                liked_tracks: parse::library_track_ids(&liked?).into_iter().collect(),
                disliked_tracks: parse::library_track_ids(&disliked.unwrap_or_default()).into_iter().collect(),
                liked_albums: ids(albums, |value| parse::album(value).map(|album| album.id)),
                liked_artists: ids(artists, |value| parse::artist(value).map(|artist| artist.id)),
                disliked_artists: ids(disliked_artists, |value| parse::artist(value).map(|artist| artist.id)),
                liked_playlists: parse::liked_keys(&playlists.unwrap_or_default(), parse::liked_playlist_key)
                    .into_iter()
                    .collect(),
                pinned: parse::pins(&pins.unwrap_or_default()),
            })
        })
    }

    fn set_pin(&self, target: &Target, on: bool) -> Result<()> {
        let (kind, body) = match target {
            Target::Album(id) => ("album", json!({ "id": id })),
            Target::Artist(id) => ("artist", json!({ "id": id })),
            Target::Playlist(key) => ("playlist", json!({ "uid": key.owner, "kind": key.kind })),
            Target::Track(_) => return Err(Error::Unsupported),
        };
        let method = if on { Method::PUT } else { Method::DELETE };
        net::block_on(self.send_json(method, &format!("pin/{kind}"), &body))?;
        Ok(())
    }

    fn upload(&self, uid: u64, kind: u32, name: &str, data: Vec<u8>) -> Result<()> {
        net::block_on(async {
            let (owner, playlist) = (uid.to_string(), format!("{uid}:{kind}"));
            let query = [
                ("uid", owner.as_str()),
                ("playlist-id", playlist.as_str()),
                ("path", name),
                ("visibility", "private"),
            ];
            let slot = send(self.http.post(url("loader/upload-url")).query(&query)).await?;
            let target = slot.text("post-target");
            if target.is_empty() {
                return Err(Error::Api("no upload target".into()));
            }
            let request = self
                .http
                .post(target)
                .header(CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
                .body(multipart(name, data));
            match request.send().await {
                Ok(response) if response.status().is_success() || response.status() == StatusCode::GATEWAY_TIMEOUT => {
                    Ok(())
                }
                Ok(response) => Err(Error::Status { status: response.status().as_u16(), message: String::new() }),
                Err(error) if error.is_timeout() => Ok(()),
                Err(error) => Err(error.into()),
            }
        })
    }

    fn playlists(&self, uid: u64) -> Result<Vec<Playlist>> {
        Ok(parse::playlists(&net::block_on(self.get(&format!("users/{uid}/playlists/list"), &[]))?))
    }

    fn set_relation(&self, uid: u64, target: &Target, relation: Relation, on: bool) -> Result<()> {
        let client = &self.client;
        let album_id = |id: &str| id.parse::<u32>().map_err(|_| Error::NotFound(id.to_owned()));
        net::block_on(async {
            match (target, relation, on) {
                (Target::Track(track), Relation::Like, true) => {
                    client.add_liked_tracks(&AddLikedTracksOptions::new(uid, [track.id.as_str()])).await?;
                }
                (Target::Track(track), Relation::Like, false) => {
                    client.remove_liked_tracks(&RemoveLikedTracksOptions::new(uid, [track.id.as_str()])).await?;
                }
                (Target::Track(track), Relation::Dislike, true) => {
                    client.add_disliked_tracks(&AddDislikedTracksOptions::new(uid, [track.id.as_str()])).await?;
                }
                (Target::Track(track), Relation::Dislike, false) => {
                    client.remove_disliked_tracks(&RemoveDislikedTracksOptions::new(uid, [track.id.as_str()])).await?;
                }
                (Target::Album(id), Relation::Like, true) => {
                    client.add_liked_album(&AddLikedAlbumOptions::new(uid, album_id(id)?)).await?;
                }
                (Target::Album(id), Relation::Like, false) => {
                    client.remove_liked_album(&RemoveLikedAlbumOptions::new(uid, album_id(id)?)).await?;
                }
                (Target::Artist(id), Relation::Like, true) => {
                    client.add_liked_artist(&AddLikedArtistOptions::new(uid, id.as_str())).await?;
                }
                (Target::Artist(id), Relation::Like, false) => {
                    client.remove_liked_artist(&RemoveLikedArtistOptions::new(uid, id.as_str())).await?;
                }
                (Target::Artist(id), Relation::Dislike, true) => {
                    client.add_disliked_artist(&AddDislikedArtistOptions::new(uid, id.as_str())).await?;
                }
                (Target::Artist(id), Relation::Dislike, false) => {
                    client.remove_disliked_artist(&RemoveDislikedArtistOptions::new(uid, id.as_str())).await?;
                }
                (Target::Playlist(key), Relation::Like, true) => {
                    client.add_liked_playlist(&AddLikedPlaylistOptions::new(uid, key.owner, key.kind)).await?;
                }
                (Target::Playlist(key), Relation::Like, false) => {
                    client.remove_liked_playlist(&RemoveLikedPlaylistOptions::new(uid, key.owner, key.kind)).await?;
                }
                (Target::Album(_) | Target::Playlist(_), Relation::Dislike, _) => return Err(Error::Unsupported),
            }
            Ok::<_, Error>(())
        })
    }

    fn create_playlist(&self, uid: u64, title: &str, public: bool) -> Result<Playlist> {
        let visibility = if public { "public" } else { "private" };
        let value = net::block_on(
            self.post(&format!("users/{uid}/playlists/create"), &[("title", title), ("visibility", visibility)]),
        )?;
        parse::playlist(parse::result(&value)).ok_or_else(|| Error::Api("the playlist was not created".into()))
    }

    fn edit_playlist(&self, uid: u64, kind: u32, edit: &PlaylistEdit) -> Result<()> {
        let path = format!("users/{uid}/playlists/{kind}");
        net::block_on(async {
            match edit {
                PlaylistEdit::Rename(title) => self.post(&format!("{path}/name"), &[("value", title.as_str())]).await?,
                PlaylistEdit::Visibility(public) => {
                    let value = if *public { "public" } else { "private" };
                    self.post(&format!("{path}/visibility"), &[("value", value)]).await?
                }
                PlaylistEdit::Description(text) => {
                    self.post(&format!("{path}/description"), &[("value", text.as_str())]).await?
                }
                PlaylistEdit::Delete => self.post(&format!("{path}/delete"), &[]).await?,
                PlaylistEdit::Insert { .. }
                | PlaylistEdit::Remove(_)
                | PlaylistEdit::Restore(_)
                | PlaylistEdit::Move { .. } => {
                    let diff = playlist_diff(edit).ok_or(Error::Unsupported)?;
                    let (diff, revision) = (diff.to_string(), self.playlist_revision(uid, kind).await?.to_string());
                    self.post(
                        &format!("{path}/change-relative"),
                        &[("diff", diff.as_str()), ("revision", revision.as_str())],
                    )
                    .await?
                }
            };
            Ok::<_, Error>(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::track;

    #[test]
    fn builds_playlist_diffs() {
        let moved = playlist_diff(&PlaylistEdit::Move { from: 4, to: 1, track: track("7") }).unwrap();
        assert_eq!(moved[0]["op"], "delete");
        assert_eq!((moved[0]["from"].as_u64(), moved[0]["to"].as_u64()), (Some(4), Some(5)));
        assert_eq!((moved[1]["op"].as_str(), moved[1]["at"].as_u64()), (Some("insert"), Some(1)));
        assert_eq!(moved[1]["tracks"][0]["id"], "7");
        let inserted = playlist_diff(&PlaylistEdit::Insert { at: 3, tracks: vec![track("1"), track("2")] }).unwrap();
        assert_eq!(inserted[0]["tracks"].as_array().unwrap().len(), 2);
        let removed = playlist_diff(&PlaylistEdit::Remove(vec![(1, track("a")), (5, track("b"))])).unwrap();
        assert_eq!((removed[0]["from"].as_u64(), removed[1]["from"].as_u64()), (Some(5), Some(1)));
        let restored = playlist_diff(&PlaylistEdit::Restore(vec![(5, track("b")), (1, track("a"))])).unwrap();
        assert_eq!((restored[0]["at"].as_u64(), restored[1]["at"].as_u64()), (Some(1), Some(5)));
        assert!(playlist_diff(&PlaylistEdit::Delete).is_none());
        let body = String::from_utf8(multipart("a\"b.mp3", b"MP3".to_vec())).unwrap();
        assert!(body.contains("filename=\"a_b.mp3\"") && body.contains("\r\n\r\nMP3\r\n--"));
    }
}
