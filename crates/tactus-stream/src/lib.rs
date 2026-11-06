#![forbid(unsafe_code)]

mod buffer;
mod ts;

pub use buffer::{Buffer, Cancel, Reader};
pub use rodio::{Decoder, Source};
use std::sync::Arc;
use tactus_api::error::{Error, Result};
use tactus_api::{AudioSource, Codec};

pub struct Opened {
    pub decoder: Decoder<Reader>,
    pub cancel: Cancel,
}

pub fn fetch(source: &AudioSource) -> Result<Arc<Buffer>> {
    Ok(match source.url.strip_prefix("file:") {
        Some(path) => Buffer::complete(std::fs::read(path)?),
        None => Buffer::fetch(source.url.clone(), tactus_api::net::client_for(source.proxy.as_ref())),
    })
}

pub fn open(buffer: &Arc<Buffer>, codec: Codec) -> Result<Opened> {
    let reader = buffer.reader();
    let cancel = reader.cancel();
    let mut builder = Decoder::builder().with_data(reader).with_hint(codec.extension()).with_seekable(true);
    if let Some(total) = buffer.total() {
        builder = builder.with_byte_len(total);
    }
    let decoder = builder.build().map_err(|error| Error::Audio(error.to_string()))?;
    Ok(Opened { decoder, cancel })
}
