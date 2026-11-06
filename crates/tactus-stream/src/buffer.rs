use crate::ts::{Demuxer, segments};
use reqwest::StatusCode;
use reqwest::header::RANGE;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::Duration;
use tactus_api::error::{Error, Result};
use tactus_api::net;
use tactus_api::sync::Guard;

const RESUME_DELAYS: [Duration; 3] = [Duration::from_secs(1), Duration::from_secs(3), Duration::from_secs(10)];

#[derive(Default)]
struct State {
    data: Vec<u8>,
    total: Option<u64>,
    done: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct Buffer {
    state: Mutex<State>,
    changed: Condvar,
}

impl Buffer {
    pub fn fetch(url: String, client: reqwest::Client) -> Arc<Buffer> {
        let buffer = Arc::new(Buffer::default());
        let weak = Arc::downgrade(&buffer);
        net::spawn(async move {
            let result = if is_playlist(&url) {
                stream_playlist(&client, &url, &weak).await
            } else {
                download(&client, &url, &weak).await
            };
            if let Some(buffer) = weak.upgrade() {
                buffer.finish(result.err().map(|error| error.to_string()));
            }
        });
        buffer
    }

    pub fn complete(data: Vec<u8>) -> Arc<Buffer> {
        Arc::new(Buffer {
            state: Mutex::new(State { total: Some(data.len() as u64), data, done: true, error: None }),
            changed: Condvar::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.guard()
    }

    fn update(&self, change: impl FnOnce(&mut State)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }

    fn append(&self, chunk: &[u8]) {
        self.update(|state| state.data.extend_from_slice(chunk));
    }

    fn finish(&self, error: Option<String>) {
        self.update(|state| {
            state.done = true;
            state.error = error;
        });
    }

    fn set_total(&self, total: Option<u64>) {
        self.lock().total = total;
    }

    fn len(&self) -> u64 {
        self.lock().data.len() as u64
    }

    pub fn total(&self) -> Option<u64> {
        let state = self.lock();
        state.total.or(state.done.then_some(state.data.len() as u64))
    }

    pub fn failed(&self) -> bool {
        self.lock().error.is_some()
    }

    fn wait_for(&self, length: u64, cancelled: &AtomicBool) -> MutexGuard<'_, State> {
        self.changed
            .wait_while(self.lock(), |state| {
                (state.data.len() as u64) < length && !state.done && !cancelled.load(Ordering::Acquire)
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn reader(self: &Arc<Self>) -> Reader {
        Reader { buffer: self.clone(), position: 0, cancelled: Arc::default() }
    }
}

async fn download(client: &reqwest::Client, url: &str, buffer: &Weak<Buffer>) -> Result<()> {
    let mut failures = 0;
    loop {
        let Some(offset) = buffer.upgrade().map(|buffer| buffer.len()) else {
            return Ok(());
        };
        let Err(error) = transfer(client, url, offset, buffer).await else {
            return Ok(());
        };
        if buffer.upgrade().is_some_and(|buffer| buffer.len() > offset) {
            failures = 0;
        }
        match RESUME_DELAYS.get(failures) {
            Some(&delay) if error.is_transient() => {
                failures += 1;
                net::sleep(delay).await;
            }
            _ => return Err(error),
        }
    }
}

async fn transfer(client: &reqwest::Client, url: &str, offset: u64, buffer: &Weak<Buffer>) -> Result<()> {
    let mut request = client.get(url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let mut response = request.send().await?.error_for_status()?;
    let Some(target) = buffer.upgrade() else {
        return Ok(());
    };
    match offset {
        0 => target.set_total(response.content_length()),
        _ if response.status() != StatusCode::PARTIAL_CONTENT => {
            return Err(Error::Audio("the server cannot resume the stream".into()));
        }
        _ => {}
    }
    drop(target);
    while let Some(chunk) = response.chunk().await? {
        match buffer.upgrade() {
            Some(buffer) => buffer.append(&chunk),
            None => return Ok(()),
        }
    }
    match buffer.upgrade() {
        Some(buffer) if buffer.total().is_some_and(|total| buffer.len() < total) => {
            Err(Error::Status { status: 503, message: "the stream ended early".into() })
        }
        _ => Ok(()),
    }
}

fn is_playlist(url: &str) -> bool {
    url.split(['?', '#']).next().is_some_and(|path| path.ends_with(".m3u8"))
}

async fn stream_playlist(client: &reqwest::Client, url: &str, buffer: &Weak<Buffer>) -> Result<()> {
    let playlist = client.get(url).send().await?.error_for_status()?.text().await?;
    let mut demuxer = Demuxer::default();
    for segment in segments(&playlist, url) {
        let data = client.get(&segment).send().await?.error_for_status()?.bytes().await?;
        let mut audio = Vec::new();
        demuxer.push(&data, &mut audio);
        match buffer.upgrade() {
            Some(buffer) => buffer.append(&audio),
            None => return Ok(()),
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct Cancel {
    buffer: Arc<Buffer>,
    cancelled: Arc<AtomicBool>,
}

impl Cancel {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.buffer.update(|_| {});
    }
}

pub struct Reader {
    buffer: Arc<Buffer>,
    position: u64,
    cancelled: Arc<AtomicBool>,
}

impl Reader {
    pub fn cancel(&self) -> Cancel {
        Cancel { buffer: self.buffer.clone(), cancelled: self.cancelled.clone() }
    }
}

impl Read for Reader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let state = self.buffer.wait_for(self.position + 1, &self.cancelled);
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::other("playback stopped"));
        }
        let start = self.position as usize;
        if start >= state.data.len() {
            return match &state.error {
                Some(error) => Err(io::Error::other(error.clone())),
                None => Ok(0),
            };
        }
        let count = output.len().min(state.data.len() - start);
        output[..count].copy_from_slice(&state.data[start..start + count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for Reader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let target = match position {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self
                .buffer
                .total()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "unknown stream length"))?
                .checked_add_signed(delta),
        };
        self.position = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))?;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_seeks_in_a_complete_buffer() {
        let mut reader = Buffer::complete(b"hello world".to_vec()).reader();
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, "hello world");
        assert_eq!(reader.seek(SeekFrom::End(-5)).unwrap(), 6);
        let mut tail = [0; 5];
        reader.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, b"world");
        assert!(reader.seek(SeekFrom::Current(-20)).is_err());
    }

    #[test]
    fn recognizes_playlists() {
        assert!(is_playlist("https://host/a/00000.m3u8"));
        assert!(is_playlist("https://host/a/00000.m3u8?sign=1"));
        assert!(!is_playlist("https://host/a/track.mp3"));
    }

    #[test]
    fn waits_for_data_that_arrives_later() {
        let buffer = Arc::new(Buffer::default());
        let mut reader = buffer.reader();
        let writer = buffer.clone();
        let thread = std::thread::spawn(move || {
            writer.append(b"abc");
            writer.finish(None);
        });
        let mut data = Vec::new();
        reader.read_to_end(&mut data).unwrap();
        thread.join().unwrap();
        assert_eq!(data, b"abc");
    }

    #[test]
    fn cancelling_wakes_a_waiting_reader() {
        let buffer = Arc::new(Buffer::default());
        let mut reader = buffer.reader();
        let cancel = reader.cancel();
        let thread = std::thread::spawn(move || {
            let mut byte = [0];
            reader.read(&mut byte)
        });
        std::thread::sleep(Duration::from_millis(50));
        cancel.cancel();
        assert!(thread.join().unwrap().is_err());
        assert!(!buffer.failed());
    }

    #[test]
    fn a_failed_download_reports_the_error_after_its_data() {
        let buffer = Arc::new(Buffer::default());
        buffer.append(b"ab");
        buffer.finish(Some("connection reset".into()));
        let mut reader = buffer.reader();
        let mut data = [0; 2];
        reader.read_exact(&mut data).unwrap();
        assert!(reader.read(&mut data).is_err());
        assert!(buffer.failed());
    }
}
