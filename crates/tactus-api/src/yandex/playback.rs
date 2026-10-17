use super::*;

impl PlaybackService for YandexService {
    fn audio_source(&self, id: &str, quality: Quality) -> Result<AudioSource> {
        let (file_quality, codecs) = match quality {
            Quality::Lossless => (FileQuality::Lossless, vec![FileCodec::Flac, FileCodec::Mp3]),
            Quality::High => (FileQuality::Normal, vec![FileCodec::Mp3]),
            Quality::Low => (FileQuality::Low, vec![FileCodec::Mp3]),
        };
        let options = GetFileInfoOptions::new(id).quality(file_quality).codecs(codecs);
        net::block_on(async {
            if let Ok(info) = self.client.get_file_info(&options).await
                && !info.url.is_empty()
            {
                return Ok(AudioSource { url: info.url, codec: Codec::parse(&info.codec) });
            }
            let variants =
                self.client.get_download_info(&GetDownloadInfoOptions::new(id)).await.map_err(Error::from)?;
            let best = variants
                .iter()
                .filter(|variant| variant.codec == "mp3" && !variant.preview)
                .max_by_key(|variant| variant.bitrate_in_kbps)
                .ok_or_else(|| Error::NotFound(id.to_owned()))?;
            Ok::<_, Error>(AudioSource { url: best.get_direct_link(&self.http).await?, codec: Codec::Mp3 })
        })
    }

    fn report_play(&self, uid: u64, play: &Play) -> Result<()> {
        let now = chrono::Utc::now();
        let length = (play.track.duration_ms / 1000) as u32;
        let mut options = PlayAudioOptions::new(play.from.clone())
            .track_id(play.track.id.clone())
            .uid(uid)
            .track_length_seconds(length)
            .total_played_seconds(play.played_seconds)
            .end_position_seconds(play.played_seconds.min(length))
            .timestamp(now)
            .client_now(now);
        if !play.track.album_id.is_empty() {
            options = options.album_id(play.track.album_id.clone());
        }
        Ok(net::block_on(self.client.play_audio(&options))?)
    }

    fn start_wave(&self, seeds: &[String]) -> Result<WaveBatch> {
        let body = json!({ "seeds": seeds, "includeTracksInResponse": true, "interactive": true });
        let batch = parse::wave_batch(&net::block_on(self.post_json("rotor/session/new", &body))?, "");
        if batch.tracks.is_empty() {
            return Err(Error::NotFound(seeds.join(",")));
        }
        Ok(batch)
    }

    fn wave_tracks(&self, session: &str, queue: &[String]) -> Result<WaveBatch> {
        let value =
            net::block_on(self.post_json(&format!("rotor/session/{session}/tracks"), &json!({ "queue": queue })))?;
        Ok(parse::wave_batch(&value, session))
    }

    fn wave_feedback(&self, session: &str, batch: &str, event: &WaveEvent) -> Result<()> {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut body = match event {
            WaveEvent::RadioStarted => json!({ "type": "radioStarted" }),
            WaveEvent::TrackStarted { track_id } => json!({ "type": "trackStarted", "trackId": track_id }),
            WaveEvent::TrackFinished { track_id, seconds } => {
                json!({ "type": "trackFinished", "trackId": track_id, "totalPlayedSeconds": seconds })
            }
            WaveEvent::Skip { track_id, seconds } => {
                json!({ "type": "skip", "trackId": track_id, "totalPlayedSeconds": seconds })
            }
        };
        body["timestamp"] = json!(timestamp);
        let body = json!({ "event": body, "batchId": (!batch.is_empty()).then_some(batch) });
        net::block_on(self.post_json(&format!("rotor/session/{session}/feedback"), &body))?;
        Ok(())
    }

    fn active_queue(&self) -> Result<Option<RemoteQueue>> {
        net::block_on(async {
            let Some(id) = parse::queue_id(&self.get("queues", &[]).await?) else {
                return Ok(None);
            };
            let queue = self.get(&format!("queues/{id}"), &[]).await?;
            Ok::<_, Error>(Some(parse::remote_queue(id, &queue)))
        })
    }

    fn create_queue(&self, tracks: &[Track], index: usize, context: &str) -> Result<String> {
        let body = json!({
            "context": { "type": "various", "id": context, "description": context },
            "tracks": tracks.iter().map(|track| json!({
                "trackId": track.id,
                "albumId": track.album_id,
                "from": "desktop_win-own_tracks-track-default",
            })).collect::<Vec<_>>(),
            "currentIndex": index,
            "isInteractive": true,
            "from": "desktop_win",
        });
        let value = net::block_on(self.post_json("queues", &body))?;
        match parse::result(&value).id("id") {
            id if id.is_empty() => Err(Error::Api("the queue was not created".into())),
            id => Ok(id),
        }
    }

    fn update_queue_position(&self, id: &str, index: usize) -> Result<()> {
        let path = format!("queues/{id}/update-position");
        net::block_on(send(
            self.http
                .post(url(&path))
                .query(&[("currentIndex", index.to_string().as_str()), ("isInteractive", "true")])
                .header("X-Yandex-Music-Device", DEVICE),
        ))?;
        Ok(())
    }
}
