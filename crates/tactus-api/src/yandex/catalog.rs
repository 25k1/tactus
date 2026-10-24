use super::*;

impl YandexService {
    async fn search(&self, query: &str) -> Result<Page> {
        let (all, podcasts) = tokio::join!(
            self.get("search", &[("text", query), ("type", "all"), ("page", "0"), ("nococrrect", "false")]),
            self.get("search", &[("text", query), ("type", "podcast"), ("page", "0")]),
        );
        Ok(parse::search(&all?, &podcasts.unwrap_or_default()))
    }

    async fn for_you(&self) -> Result<Page> {
        let (feed, landing, chart, tree) = tokio::join!(
            self.get("feed", &[]),
            self.get("landing3", &[("blocks", "personal-playlists,new-releases,new-playlists,chart,mixes")]),
            self.get("landing3/chart/russia", &[]),
            self.get("metatags", &[]),
        );
        match (feed, landing, chart) {
            (Err(error), Err(_), Err(_)) => Err(error),
            (feed, landing, chart) => {
                let mut page = parse::for_you(feed.as_ref().ok(), landing.as_ref().ok(), chart.as_ref().ok());
                if let Ok(tree) = &tree {
                    page.sections.push(parse::genres(tree));
                }
                Ok(page)
            }
        }
    }

    async fn collection(&self, uid: u64) -> Result<Page> {
        let base = format!("users/{uid}");
        let (liked, disliked, albums, artists, playlists, disliked_artists, pins) = tokio::join!(
            self.get(&format!("{base}/likes/tracks"), &[]),
            self.get(&format!("{base}/dislikes/tracks"), &[]),
            self.get(&format!("{base}/likes/albums"), &[("rich", "true")]),
            self.get(&format!("{base}/likes/artists"), &[("with-timestamps", "false")]),
            self.get(&format!("{base}/likes/playlists"), &[]),
            self.get(&format!("{base}/dislikes/artists"), &[]),
            self.get("pins", &[]),
        );
        let liked_ids = parse::library_track_ids(&liked?);
        let disliked_ids = parse::library_track_ids(&disliked.unwrap_or_default());
        let (liked, disliked) = tokio::join!(self.fetch_tracks(&liked_ids), self.fetch_tracks(&disliked_ids));
        let mut page = parse::collection(
            liked?,
            &albums.unwrap_or_default(),
            &artists.unwrap_or_default(),
            &playlists.unwrap_or_default(),
            disliked.unwrap_or_default(),
            &disliked_artists.unwrap_or_default(),
        );
        let pinned = parse::pins(&pins.unwrap_or_default());
        if !pinned.is_empty() {
            page.sections.insert(0, Section::new(SectionKind::Pinned, "", pinned));
        }
        Ok(page)
    }

    async fn artist(&self, id: &str) -> Result<Page> {
        let (brief, tracks, albums) = tokio::join!(
            self.get(&format!("artists/{id}/brief-info"), &[]),
            self.get(&format!("artists/{id}/tracks"), &[("page", "0"), ("page-size", "100")]),
            self.get(
                &format!("artists/{id}/direct-albums"),
                &[("page", "0"), ("page-size", "100"), ("sort-by", "year")]
            ),
        );
        Ok(parse::artist_page(&brief?, tracks.as_ref().ok(), albums.as_ref().ok()))
    }

    async fn playlist(&self, key: PlaylistKey, uid: u64) -> Result<Page> {
        let path = format!("users/{}/playlists/{}", key.owner, key.kind);
        let suggestions = async {
            if key.owner == uid { self.get(&format!("{path}/recommendations"), &[]).await.ok() } else { None }
        };
        let (playlist, suggestions) = tokio::join!(self.get(&path, &[("rich-tracks", "true")]), suggestions);
        Ok(parse::playlist_page(&playlist?, suggestions.as_ref()))
    }

    async fn tag(&self, id: &str) -> Result<Page> {
        let (title, keys) = parse::tag_ids(&self.get(&format!("tags/{id}/playlist-ids"), &[]).await?);
        let playlists = self.fetch_playlists(&keys).await?;
        Ok(Page {
            title,
            sections: vec![Section::new(
                SectionKind::Playlists,
                "",
                playlists.into_iter().map(Item::Playlist).collect(),
            )],
            ..Page::default()
        })
    }

    async fn lyrics_text(&self, id: &str, format: LyricsFormat) -> Result<Option<Lyrics>> {
        let lyrics = self.client.get_lyrics(&GetLyricsOptions::new(id, format)).await?;
        if lyrics.download_url.is_empty() {
            return Ok(None);
        }
        let text = net::client().get(&lyrics.download_url).send().await?.error_for_status()?.text().await?;
        let lines = parse::lyrics(&text);
        Ok((!lines.is_empty()).then_some(Lyrics { lines, writers: lyrics.writers }))
    }
}

impl CatalogService for YandexService {
    fn page(&self, view: &View, uid: u64) -> Result<Page> {
        net::block_on(async {
            Ok::<_, Error>(match view {
                View::Search(query) => self.search(query).await?,
                View::Waves => parse::waves(&self.get("landing-blocks/waves", &[]).await?),
                View::ForYou => self.for_you().await?,
                View::Collection => self.collection(uid).await?,
                View::Playlists => Page {
                    sections: vec![Section::new(
                        SectionKind::Playlists,
                        "",
                        parse::playlists(&self.get(&format!("users/{uid}/playlists/list"), &[]).await?)
                            .into_iter()
                            .map(Item::Playlist)
                            .collect(),
                    )],
                    ..Page::default()
                },
                View::Radio => {
                    let (dashboard, stations) = tokio::join!(
                        self.get("rotor/stations/dashboard", &[]),
                        self.get("rotor/stations/list", &[("language", net::language())]),
                    );
                    parse::radio(dashboard.as_ref().ok(), &stations?)
                }
                View::Podcasts => {
                    let (catalogue, episodes) =
                        tokio::join!(self.get("non-music/catalogue", &[]), self.get("non-music/new-episodes", &[]));
                    let mut page = parse::non_music(&catalogue?);
                    page.sections.splice(0..0, episodes.as_ref().map(parse::new_episodes).unwrap_or_default());
                    page
                }
                View::Category(id) => parse::non_music(&self.get(&format!("non-music/category/{id}"), &[]).await?),
                View::History => parse::history(&self.get("music-history", &[("fullModelsCount", "200")]).await?),
                View::Album(id) => parse::album_page(&self.get(&format!("albums/{id}/with-tracks"), &[]).await?),
                View::Artist(id) => self.artist(id).await?,
                View::Playlist(key) => self.playlist(*key, uid).await?,
                View::Track(id) => {
                    let (full, similar) = tokio::join!(
                        self.get(&format!("tracks/{id}/full-info"), &[]),
                        self.get(&format!("tracks/{id}/similar"), &[]),
                    );
                    parse::track_page(&full?, similar.as_ref().ok())
                }
                View::User(login) => {
                    parse::user_page(login, &self.get(&format!("users/{login}/playlists/list"), &[]).await?)
                }
                View::Chart(region) => parse::chart_page(&self.get(&format!("landing3/chart/{region}"), &[]).await?),
                View::Tag(id) => self.tag(id).await?,
                View::Genre(id) => {
                    let albums = format!("metatags/{id}/albums");
                    let (main, popular, fresh, tree) = tokio::join!(
                        self.get(&format!("metatags/{id}"), &[]),
                        self.get(&albums, &[("sortBy", "popular"), ("offset", "0")]),
                        self.get(&albums, &[("sortBy", "new"), ("offset", "0")]),
                        self.get("metatags", &[]),
                    );
                    parse::genre_page(
                        id,
                        &main?,
                        &popular.unwrap_or_default(),
                        &fresh.unwrap_or_default(),
                        tree.as_ref().ok(),
                    )
                }
            })
        })
    }

    fn more(&self, view: &View, kind: SectionKind, page: u32) -> Result<Paged> {
        let number = page.to_string();
        net::block_on(async {
            match (view, kind) {
                (View::Search(query), kind) => {
                    let (search, _) = parse::search_type(kind).ok_or(Error::Unsupported)?;
                    let value = self
                        .get("search", &[("text", query.as_str()), ("type", search), ("page", number.as_str())])
                        .await?;
                    Ok(parse::search_more(&value, kind, page))
                }
                (View::Artist(id), SectionKind::Tracks | SectionKind::Albums) => {
                    let size = parse::ARTIST_PAGE.to_string();
                    let paging = [("page", number.as_str()), ("page-size", size.as_str())];
                    let value = if kind == SectionKind::Tracks {
                        self.get(&format!("artists/{id}/tracks"), &paging).await?
                    } else {
                        let query = [paging[0], paging[1], ("sort-by", "year")];
                        self.get(&format!("artists/{id}/direct-albums"), &query).await?
                    };
                    Ok(parse::artist_more(&value, kind))
                }
                (View::Genre(id), SectionKind::Albums | SectionKind::GenreReleases) => {
                    let sort = if kind == SectionKind::Albums { "popular" } else { "new" };
                    let offset = (page * parse::GENRE_PAGE).to_string();
                    let value = self
                        .get(&format!("metatags/{id}/albums"), &[("sortBy", sort), ("offset", offset.as_str())])
                        .await?;
                    Ok(parse::genre_more(&value, page))
                }
                _ => Err(Error::Unsupported),
            }
        })
    }

    fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        net::block_on(self.fetch_tracks(ids))
    }

    fn suggestions(&self, part: &str) -> Result<Suggestions> {
        Ok(parse::suggestions(&net::block_on(self.get("search/suggest", &[("part", part)]))?))
    }

    fn track_info(&self, id: &str) -> Result<TrackInfo> {
        parse::track_info(&net::block_on(self.get(&format!("tracks/{id}/full-info"), &[]))?)
            .ok_or_else(|| Error::NotFound(id.to_owned()))
    }

    fn lyrics(&self, id: &str) -> Result<Option<Lyrics>> {
        net::block_on(async {
            match self.lyrics_text(id, LyricsFormat::LRC).await {
                Ok(Some(lyrics)) => Ok(Some(lyrics)),
                _ => Ok(self.lyrics_text(id, LyricsFormat::TEXT).await.ok().flatten()),
            }
        })
    }
}
