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
                return Ok(AudioSource::new(info.url, Codec::parse(&info.codec)).via(self.proxy.as_ref()));
            }
            let variants = self.client.get_download_info(&GetDownloadInfoOptions::new(id)).await.api()?;
            let best = variants
                .iter()
                .filter(|variant| variant.codec == "mp3" && !variant.preview)
                .max_by_key(|variant| variant.bitrate_in_kbps)
                .ok_or_else(|| Error::NotFound(id.to_owned()))?;
            let url = best.get_direct_link(&self.http).await.api()?;
            Ok::<_, Error>(AudioSource::new(url, Codec::Mp3).via(self.proxy.as_ref()))
        })
    }

    fn report_play(&self, play: &Play) -> Result<()> {
        let uid = net::block_on(self.uid())?;
        let form = play_form(play, uid, chrono::Utc::now());
        let form: Vec<(&str, &str)> = form.iter().map(|(key, value)| (*key, value.as_str())).collect();
        net::block_on(self.post("play-audio", &form))?;
        Ok(())
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
            WaveEvent::Like { track_id } => json!({ "type": "like", "trackId": track_id }),
            WaveEvent::Unlike { track_id } => json!({ "type": "unlike", "trackId": track_id }),
            WaveEvent::Dislike { track_id, seconds } => {
                json!({ "type": "dislike", "trackId": track_id, "totalPlayedSeconds": seconds })
            }
            WaveEvent::Undislike { track_id } => json!({ "type": "undislike", "trackId": track_id }),
        };
        body["timestamp"] = json!(timestamp);
        let body = json!({ "event": body, "batchId": (!batch.is_empty()).then_some(batch) });
        net::block_on(self.post_json(&format!("rotor/session/{session}/feedback"), &body))?;
        Ok(())
    }

    fn wave_settings(&self) -> Result<WaveSettings> {
        Ok(parse::wave_settings(&net::block_on(self.get("rotor/wave/settings", &[]))?))
    }

    fn last_wave(&self) -> Result<LastWave> {
        Ok(parse::last_wave(&net::block_on(self.get("rotor/wave/last", &[]))?))
    }

    fn reset_last_wave(&self) -> Result<()> {
        net::block_on(self.post("rotor/wave/last/reset", &[]))?;
        Ok(())
    }

    fn generated_stream(&self, station: &str) -> Result<GeneratedStream> {
        let value = net::block_on(self.get(&format!("rotor/station/{station}/stream"), &[]))?;
        let stream = parse::generated_stream(station, &value);
        if stream.url.is_empty() {
            return Err(Error::NotFound(station.to_owned()));
        }
        Ok(stream)
    }

    fn generated_feedback(&self, station: &str, stream: &str, event: StreamEvent) -> Result<()> {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let path = format!("rotor/station/{station}/feedback?streamId={}", encode_component(stream));
        net::block_on(self.post_json(&path, &json!({ "type": event.name(), "timestamp": timestamp })))?;
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

fn play_form(play: &Play, uid: u64, now: chrono::DateTime<chrono::Utc>) -> Vec<(&'static str, String)> {
    let length = (play.track.duration_ms / 1000) as u32;
    let timestamp = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut form = vec![
        ("track-id", play.track.id.clone()),
        ("from", play.from.clone()),
        ("from-cache", "false".to_owned()),
        ("play-id", String::new()),
        ("uid", uid.to_string()),
        ("timestamp", timestamp.clone()),
        ("client-now", timestamp),
        ("track-length-seconds", length.to_string()),
        ("total-played-seconds", play.played_seconds.to_string()),
        ("end-position-seconds", play.played_seconds.min(length).to_string()),
    ];
    if !play.track.album_id.is_empty() {
        form.push(("album-id", play.track.album_id.clone()));
    }
    form
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_plays_as_a_form() {
        let track = Track { id: "42".into(), album_id: "7".into(), duration_ms: 185_500, ..Track::default() };
        let play = Play { track, played_seconds: 200, from: "desktop".into() };
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let form = play_form(&play, 9, now);
        let value = |key: &str| form.iter().find(|(name, _)| *name == key).map(|(_, value)| value.as_str());
        assert_eq!(value("track-id"), Some("42"));
        assert_eq!(value("album-id"), Some("7"));
        assert_eq!(value("uid"), Some("9"));
        assert_eq!(value("track-length-seconds"), Some("185"));
        assert_eq!(value("end-position-seconds"), Some("185"));
        assert_eq!(value("timestamp"), Some("2023-11-14T22:13:20.000Z"));
    }
}
