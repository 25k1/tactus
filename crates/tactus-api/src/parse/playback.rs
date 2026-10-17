use super::*;

pub fn track_info(value: &Value) -> Option<TrackInfo> {
    let value = result(value);
    let raw = value.at("track");
    let track = track(raw)?;
    let album = raw.at("/albums/0");
    let lyrics = raw.at("lyricsInfo");
    Some(TrackInfo {
        artists: raw
            .list("artists")
            .iter()
            .map(|artist| artist.text("name").to_owned())
            .filter(|name| !name.is_empty())
            .collect(),
        album_type: album.text("type").to_owned(),
        release_date: album.text("releaseDate").get(..10).unwrap_or_default().to_owned(),
        labels: album
            .list("labels")
            .iter()
            .map(|label| label.as_str().unwrap_or_else(|| label.text("name")).to_owned())
            .filter(|label| !label.is_empty())
            .collect(),
        lyrics: lyrics.flag("hasAvailableTextLyrics") || raw.flag("lyricsAvailable"),
        synced_lyrics: lyrics.flag("hasAvailableSyncLyrics"),
        track,
    })
}

pub fn lyrics(text: &str) -> Vec<LyricLine> {
    text.lines()
        .filter_map(|line| {
            let mut rest = line.trim();
            let mut time_ms = None;
            while let Some(stripped) = rest.strip_prefix('[') {
                let (stamp, tail) = stripped.split_once(']')?;
                time_ms = time_ms.or_else(|| timestamp(stamp));
                rest = tail.trim_start();
            }
            let text = rest.trim_end();
            (!text.is_empty()).then(|| LyricLine { time_ms, text: text.to_owned() })
        })
        .collect()
}

fn timestamp(stamp: &str) -> Option<u64> {
    let (minutes, seconds) = stamp.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok().filter(|seconds: &f64| (0.0..60.0).contains(seconds))?;
    minutes.checked_mul(60_000)?.checked_add((seconds * 1000.0).round() as u64)
}

pub fn wave_batch(value: &Value, session: &str) -> WaveBatch {
    let value = result(value);
    let sequence: Vec<Value> = value.list("sequence").to_vec();
    WaveBatch {
        session: match value.text("radioSessionId") {
            "" => session.to_owned(),
            id => id.to_owned(),
        },
        batch: value.text("batchId").to_owned(),
        tracks: tracks(&sequence),
    }
}

pub fn queue_id(value: &Value) -> Option<String> {
    non_empty(result(value).at("/queues/0").id("id"))
}

pub fn remote_queue(id: String, value: &Value) -> RemoteQueue {
    let value = result(value);
    RemoteQueue {
        id,
        track_ids: value
            .list("tracks")
            .iter()
            .map(|track| match track.id("trackId") {
                id if id.is_empty() => track.id("id"),
                id => id,
            })
            .filter(|id| !id.is_empty())
            .collect(),
        index: value.num("currentIndex") as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_track_info() {
        let info = track_info(&fixture("full_info")).unwrap();
        assert_eq!(info.track.id, "1710808");
        assert!(!info.artists.is_empty());
    }

    #[test]
    fn parses_lrc_lyrics() {
        let lines = lyrics("[00:01.00] Hello\n[00:02.50][00:03.00]World\n\nplain");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time_ms, Some(1000));
        assert_eq!(lines[1], LyricLine { time_ms: Some(2500), text: "World".into() });
        assert_eq!(lines[2].time_ms, None);
        let lyrics = Lyrics { lines, writers: Vec::new() };
        assert_eq!(lyrics.line_at(0), None);
        assert_eq!(lyrics.line_at(2600), Some(1));
    }

    proptest::proptest! {
        #[test]
        fn lyrics_parsing_never_panics(text in "\\PC{0,300}") {
            let lines = lyrics(&text);
            proptest::prop_assert!(lines.iter().all(|line| !line.text.trim().is_empty()));
        }

        #[test]
        fn timestamps_stay_in_range(minutes: u64, seconds: f64) {
            let lines = lyrics(&format!("[{minutes}:{seconds}] line"));
            let time = lines.first().and_then(|line| line.time_ms);
            proptest::prop_assert!(time.is_none_or(|time| time / 60_000 == minutes && seconds.is_finite() && seconds >= 0.0));
        }
    }

    #[test]
    fn parses_queues_and_waves() {
        let queues = serde_json::json!({"result": {"queues": [{"id": "q1"}]}});
        assert_eq!(queue_id(&queues).as_deref(), Some("q1"));
        let queue = remote_queue(
            "q1".into(),
            &serde_json::json!({"result": {"currentIndex": 1, "tracks": [{"trackId": "1", "albumId": "2"}, {"trackId": 3}]}}),
        );
        assert_eq!(queue.track_ids, ["1", "3"]);
        assert_eq!(queue.index, 1);
        let batch = wave_batch(
            &serde_json::json!({"result": {"batchId": "b", "sequence": [{"track": result(&fixture("tracks"))[0].clone()}]}}),
            "s",
        );
        assert_eq!((batch.session.as_str(), batch.batch.as_str(), batch.tracks.len()), ("s", "b", 1));
    }
}
