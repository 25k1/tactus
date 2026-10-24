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
        let presaved: Vec<Item> = self.presaves(uid).await.unwrap_or_default().into_iter().map(Item::Album).collect();
        if !presaved.is_empty() {
            page.sections.push(Section::new(SectionKind::Presaves, "", presaved));
        }
        Ok(page)
    }

    async fn artist(&self, id: &str) -> Result<Page> {
        let albums = [("page", "0"), ("page-size", "100"), ("sort-by", "year")];
        let path = |tail: &str| format!("artists/{id}/{tail}");
        let (brief, tracks, direct, discography, also, similar, links, about, concerts, disclaimer) = tokio::join!(
            self.get(&path("brief-info"), &[]),
            self.get(&path("tracks"), &[("page", "0"), ("page-size", "100")]),
            self.get(&path("direct-albums"), &albums),
            self.get(&path("discography-albums"), &albums),
            self.get(&path("also-albums"), &albums),
            self.get(&path("similar"), &[]),
            self.get(&path("artist-links"), &[]),
            self.get(&path("about-artist"), &[]),
            self.get(&path("concerts"), &[]),
            self.get(&path("disclaimer"), &[]),
        );
        let mut page = parse::artist_page(&parse::ArtistData {
            brief: brief?,
            tracks: tracks.ok(),
            albums: direct.ok(),
            discography: discography.ok(),
            also: also.ok(),
            similar: similar.ok(),
            links: links.ok(),
            about: about.ok(),
            concerts: concerts.ok(),
        });
        page.add_disclaimer(&disclaimer.map(|value| parse::disclaimer(&value)).unwrap_or_default());
        Ok(page)
    }

    async fn playlist(&self, key: PlaylistKey, uid: u64) -> Result<Page> {
        let path = format!("users/{}/playlists/{}", key.owner, key.kind);
        let suggestions = async {
            if key.owner == uid { self.get(&format!("{path}/recommendations"), &[]).await.ok() } else { None }
        };
        let (playlist, suggestions) = tokio::join!(self.get(&path, &[("rich-tracks", "true")]), suggestions);
        let playlist = playlist?;
        let mut page = parse::playlist_page(&playlist, suggestions.as_ref());
        let uuid = parse::result(&playlist).text("playlistUuid");
        if !uuid.is_empty() {
            let uuid = uuid.strip_prefix("lk.").unwrap_or(uuid);
            let similar = self.get(&format!("playlist/{uuid}/similar-entities"), &[]).await;
            page.add_similar(similar.map(|value| parse::similar_entities(&value)).unwrap_or_default());
        }
        Ok(page)
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
                    let stream = |index: usize| {
                        let station = GENERATED_STATIONS[index];
                        let request = self.get(&format!("rotor/station/{station}/stream"), &[]);
                        async move { request.await.ok().map(|value| parse::generated_stream(station, &value)) }
                    };
                    let (dashboard, stations, settings, focus, energy, calm, relax) = tokio::join!(
                        self.get("rotor/stations/dashboard", &[]),
                        self.get("rotor/stations/list", &[("language", net::language())]),
                        self.get("rotor/wave/settings", &[]),
                        stream(0),
                        stream(1),
                        stream(2),
                        stream(3),
                    );
                    let mut page = parse::radio(dashboard.as_ref().ok(), &stations?);
                    let streams: Vec<GeneratedStream> = [focus, energy, calm, relax].into_iter().flatten().collect();
                    let moments = settings.as_ref().map(parse::wave_settings).unwrap_or_default().contexts;
                    let extra = [
                        (SectionKind::Moments, moments.into_iter().map(Item::Station).collect::<Vec<_>>()),
                        (SectionKind::Generated, streams.iter().map(parse::generated_station).collect()),
                    ];
                    let extra = extra.into_iter().filter(|(_, items)| !items.is_empty());
                    page.sections.splice(0..0, extra.map(|(kind, items)| Section::new(kind, "", items)));
                    page
                }
                View::Podcasts => {
                    let (catalogue, episodes, podcasts) = tokio::join!(
                        self.get("non-music/catalogue", &[]),
                        self.get("non-music/new-episodes", &[]),
                        self.get("landing3/podcasts", &[]),
                    );
                    let mut page = parse::non_music(&catalogue?);
                    page.sections.splice(0..0, episodes.as_ref().map(parse::new_episodes).unwrap_or_default());
                    let ids = podcasts.map(|value| parse::landing_ids(&value, "podcasts")).unwrap_or_default();
                    let (first, next) = parse::page_of(&ids, 0);
                    let items = self.fetch_albums(&first).await.unwrap_or_default();
                    if !items.is_empty() {
                        page.sections.push(Section::new(SectionKind::Podcasts, "", items).with_more(next));
                    }
                    page
                }
                View::Category(id) => parse::non_music(&self.get(&format!("non-music/category/{id}"), &[]).await?),
                View::History => parse::history(&self.get("music-history", &[("fullModelsCount", "200")]).await?),
                View::Album(id) => {
                    let (album, similar, disclaimer) = tokio::join!(
                        self.get(&format!("albums/{id}/with-tracks"), &[]),
                        self.get(&format!("albums/{id}/similar-entities"), &[]),
                        self.get(&format!("albums/{id}/disclaimer"), &[]),
                    );
                    let mut page = parse::album_page(&album?);
                    page.add_disclaimer(&disclaimer.map(|value| parse::disclaimer(&value)).unwrap_or_default());
                    page.add_similar(similar.map(|value| parse::similar_entities(&value)).unwrap_or_default());
                    page
                }
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
                    let (main, popular, fresh, tree, artists, playlists) = tokio::join!(
                        self.get(&format!("metatags/{id}"), &[]),
                        self.get(&albums, &[("sortBy", "popular"), ("offset", "0")]),
                        self.get(&albums, &[("sortBy", "new"), ("offset", "0")]),
                        self.get("metatags", &[]),
                        self.genre_list(id, SectionKind::Artists, 0),
                        self.genre_list(id, SectionKind::Playlists, 0),
                    );
                    parse::genre_page(
                        id,
                        &parse::GenreData {
                            main: main?,
                            popular: popular.unwrap_or_default(),
                            fresh: fresh.unwrap_or_default(),
                            tree: tree.ok(),
                            artists: artists.ok(),
                            playlists: playlists.ok(),
                        },
                    )
                }
                View::Label(id) => {
                    let paging = [("page", "0"), ("pageSize", "100")];
                    let (main, albums, artists) = tokio::join!(
                        self.get(&format!("labels/{id}"), &[]),
                        self.get(&format!("labels/{id}/albums"), &paging),
                        self.get(&format!("labels/{id}/artists"), &paging),
                    );
                    parse::label_page(&main?, albums.as_ref().ok(), artists.as_ref().ok())
                }
                View::Concerts(location) => {
                    let query: Vec<(&str, &str)> =
                        Some(("locations", location.as_str())).filter(|(_, id)| !id.is_empty()).into_iter().collect();
                    parse::concert_page(&self.get("concerts/feed", &query).await?)
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
                (
                    View::Artist(id),
                    SectionKind::Tracks | SectionKind::Albums | SectionKind::Discography | SectionKind::AppearsOn,
                ) => {
                    let size = parse::ARTIST_PAGE.to_string();
                    let paging = [("page", number.as_str()), ("page-size", size.as_str())];
                    let tail = match kind {
                        SectionKind::Tracks => "tracks",
                        SectionKind::Discography => "discography-albums",
                        SectionKind::AppearsOn => "also-albums",
                        _ => "direct-albums",
                    };
                    let path = format!("artists/{id}/{tail}");
                    let value = if kind == SectionKind::Tracks {
                        self.get(&path, &paging).await?
                    } else {
                        self.get(&path, &[paging[0], paging[1], ("sort-by", "year")]).await?
                    };
                    Ok(parse::artist_more(&value, kind))
                }
                (View::Genre(id), SectionKind::Artists | SectionKind::Playlists) => {
                    let value = self.genre_list(id, kind, page).await?;
                    Ok(if kind == SectionKind::Artists {
                        parse::genre_artists(&value, page)
                    } else {
                        parse::genre_playlists(&value, page)
                    })
                }
                (View::Label(id), SectionKind::Albums | SectionKind::Artists) => {
                    let tail = if kind == SectionKind::Albums { "albums" } else { "artists" };
                    let paging = [("page", number.as_str()), ("pageSize", "100")];
                    Ok(parse::label_more(&self.get(&format!("labels/{id}/{tail}"), &paging).await?, kind))
                }
                (View::ForYou, SectionKind::NewReleases) => {
                    let ids = parse::landing_ids(&self.get("landing3/new-releases", &[]).await?, "newReleases");
                    let (ids, next) = parse::page_of(&ids, page);
                    Ok(Paged { items: self.fetch_albums(&ids).await?, next })
                }
                (View::ForYou, SectionKind::NewPlaylists) => {
                    let keys = parse::landing_playlists(&self.get("landing3/new-playlists", &[]).await?);
                    let (keys, next) = parse::page_of(&keys, page);
                    let items = self.fetch_playlists(&keys).await?.into_iter().map(Item::Playlist).collect();
                    Ok(Paged { items, next })
                }
                (View::Podcasts, SectionKind::Podcasts) => {
                    let ids = parse::landing_ids(&self.get("landing3/podcasts", &[]).await?, "podcasts");
                    let (ids, next) = parse::page_of(&ids, page);
                    Ok(Paged { items: self.fetch_albums(&ids).await?, next })
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

    fn trailer(&self, target: &Target) -> Result<Vec<Track>> {
        let path = match target {
            Target::Album(id) => format!("albums/{id}/trailer"),
            Target::Artist(id) => format!("artists/{id}/trailer"),
            Target::Playlist(key) => format!("users/{}/playlists/{}/trailer", key.owner, key.kind),
            Target::Track(track) => format!("tracks/{}/trailer", track.id),
            Target::Wave(_) => return Err(Error::Unsupported),
        };
        let tracks = parse::trailer(&net::block_on(self.get(&path, &[]))?);
        if tracks.is_empty() {
            return Err(Error::NotFound(path));
        }
        Ok(tracks)
    }

    fn concert_locations(&self) -> Result<Vec<ConcertLocation>> {
        Ok(parse::concert_locations(&net::block_on(self.get("concerts/locations", &[]))?))
    }

    fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        net::block_on(self.fetch_tracks(ids))
    }

    fn suggestions(&self, part: &str) -> Result<Suggestions> {
        Ok(parse::suggestions(&net::block_on(self.get("search/suggest", &[("part", part)]))?))
    }

    fn track_info(&self, id: &str) -> Result<TrackInfo> {
        net::block_on(async {
            let (full, credits, disclaimer) = tokio::join!(
                self.get(&format!("tracks/{id}/full-info"), &[]),
                self.get(&format!("tracks/{id}/credits"), &[]),
                self.get(&format!("tracks/{id}/disclaimer"), &[]),
            );
            let mut info = parse::track_info(&full?).ok_or_else(|| Error::NotFound(id.to_owned()))?;
            info.credits = credits.map(|value| parse::credits(&value)).unwrap_or_default();
            info.disclaimer = disclaimer.map(|value| parse::disclaimer(&value)).unwrap_or_default();
            Ok(info)
        })
    }

    fn lyrics(&self, id: &str) -> Result<Option<Lyrics>> {
        net::block_on(async {
            match self.lyrics_text(id, LyricsFormat::LRC).await {
                Ok(Some(lyrics)) => Ok(Some(lyrics)),
                _ => Ok(self.lyrics_text(id, LyricsFormat::TEXT).await.ok().flatten()),
            }
        })
    }

    fn find_playlist(&self, link: &PlaylistLink) -> Result<PlaylistKey> {
        let path = match link {
            PlaylistLink::Owner { owner, kind } => format!("users/{owner}/playlists/{kind}"),
            PlaylistLink::Shared(id) => format!("playlist/{id}"),
        };
        let value = net::block_on(self.get(&path, &[]))?;
        parse::playlist_key(parse::result(&value)).ok_or(Error::NotFound(path))
    }
}
