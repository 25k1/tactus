use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};

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
    pub fn fetch(url: String) -> Arc<Buffer> {
        let buffer = Arc::new(Buffer::default());
        let weak = Arc::downgrade(&buffer);
        tactus_api::net::spawn(async move {
            let result = download(&url, &weak).await;
            if let Some(buffer) = weak.upgrade() {
                let mut state = buffer.lock();
                state.done = true;
                state.error = result.err().map(|error| error.to_string());
                drop(state);
                buffer.changed.notify_all();
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
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn append(&self, chunk: &[u8]) {
        self.lock().data.extend_from_slice(chunk);
        self.changed.notify_all();
    }

    fn set_total(&self, total: Option<u64>) {
        self.lock().total = total;
    }

    pub fn total(&self) -> Option<u64> {
        let state = self.lock();
        state.total.or(state.done.then_some(state.data.len() as u64))
    }

    fn wait_for(&self, length: u64) -> MutexGuard<'_, State> {
        self.changed
            .wait_while(self.lock(), |state| (state.data.len() as u64) < length && !state.done)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn reader(self: &Arc<Self>) -> Reader {
        Reader { buffer: self.clone(), position: 0 }
    }
}

async fn download(url: &str, buffer: &Weak<Buffer>) -> tactus_api::error::Result<()> {
    let mut response = tactus_api::net::client().get(url).send().await?.error_for_status()?;
    if let Some(buffer) = buffer.upgrade() {
        buffer.set_total(response.content_length());
    }
    while let Some(chunk) = response.chunk().await? {
        match buffer.upgrade() {
            Some(buffer) => buffer.append(&chunk),
            None => return Ok(()),
        }
    }
    Ok(())
}

pub struct Reader {
    buffer: Arc<Buffer>,
    position: u64,
}

impl Read for Reader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let state = self.buffer.wait_for(self.position + 1);
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
    fn waits_for_data_that_arrives_later() {
        let buffer = Arc::new(Buffer::default());
        let mut reader = buffer.reader();
        let writer = buffer.clone();
        let thread = std::thread::spawn(move || {
            writer.append(b"abc");
            let mut state = writer.lock();
            state.done = true;
            drop(state);
            writer.changed.notify_all();
        });
        let mut data = Vec::new();
        reader.read_to_end(&mut data).unwrap();
        thread.join().unwrap();
        assert_eq!(data, b"abc");
    }
}
