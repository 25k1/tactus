use std::io;
use std::path::Path;
use tactus_api::Track;

const PADDING: u8 = 1;
const COMMENT: u8 = 4;
const PICTURE: u8 = 6;
const FRONT_COVER: u8 = 3;
const UTF8: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Title,
    Artist,
    Album,
    Genre,
    Year,
    Number,
}

impl Field {
    fn id3(self) -> &'static [u8; 4] {
        match self {
            Field::Title => b"TIT2",
            Field::Artist => b"TPE1",
            Field::Album => b"TALB",
            Field::Genre => b"TCON",
            Field::Year => b"TDRC",
            Field::Number => b"TRCK",
        }
    }

    fn vorbis(self) -> &'static str {
        match self {
            Field::Title => "TITLE",
            Field::Artist => "ARTIST",
            Field::Album => "ALBUM",
            Field::Genre => "GENRE",
            Field::Year => "DATE",
            Field::Number => "TRACKNUMBER",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tags<'a> {
    fields: Vec<(Field, String)>,
    cover: Option<&'a [u8]>,
}

impl<'a> Tags<'a> {
    pub fn new(track: &Track, number: Option<usize>, cover: Option<&'a [u8]>) -> Self {
        let fields = [
            (Field::Title, track.full_title()),
            (Field::Artist, track.artist.clone()),
            (Field::Album, track.album.clone()),
            (Field::Genre, track.genre.clone()),
            (Field::Year, if track.year > 0 { track.year.to_string() } else { String::new() }),
            (Field::Number, number.map(|number| number.to_string()).unwrap_or_default()),
        ]
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .collect();
        Self { fields, cover }
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let tagged = self
            .apply(&std::fs::read(path)?)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unsupported audio format"))?;
        std::fs::write(path, tagged)
    }

    pub fn apply(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.starts_with(b"fLaC") {
            self.flac(data)
        } else if data.starts_with(b"ID3")
            || data.first() == Some(&0xFF) && data.get(1).is_some_and(|b| b & 0xE0 == 0xE0)
        {
            Some(self.mp3(data))
        } else {
            None
        }
    }

    fn mp3(&self, data: &[u8]) -> Vec<u8> {
        let mut frames = Vec::new();
        for (field, value) in &self.fields {
            id3_frame(&mut frames, field.id3(), &[&[UTF8], value.as_bytes()]);
        }
        if let Some(cover) = self.cover {
            id3_frame(&mut frames, b"APIC", &[&[UTF8], mime(cover).as_bytes(), &[0, FRONT_COVER, 0], cover]);
        }
        let audio = &data[id3_length(data)..];
        let mut tagged = Vec::with_capacity(10 + frames.len() + audio.len());
        tagged.extend_from_slice(b"ID3\x04\x00\x00");
        tagged.extend_from_slice(&syncsafe(frames.len()));
        tagged.extend_from_slice(&frames);
        tagged.extend_from_slice(audio);
        tagged
    }

    fn flac(&self, data: &[u8]) -> Option<Vec<u8>> {
        let comment = self.vorbis_comment();
        let picture = self.cover.filter(|cover| cover.len() < 1 << 23).map(flac_picture);
        let mut blocks: Vec<(u8, &[u8])> = Vec::new();
        let mut position = 4;
        loop {
            let header = data.get(position..position + 4)?;
            let length = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
            let body = data.get(position + 4..position + 4 + length)?;
            position += 4 + length;
            let kind = header[0] & 0x7F;
            if !matches!(kind, PADDING | COMMENT | PICTURE) {
                blocks.push((kind, body));
            }
            if header[0] & 0x80 != 0 {
                break;
            }
        }
        blocks.push((COMMENT, &comment));
        blocks.extend(picture.as_deref().map(|picture| (PICTURE, picture)));
        let mut tagged = Vec::with_capacity(data.len() + comment.len() + picture.as_ref().map_or(0, Vec::len));
        tagged.extend_from_slice(b"fLaC");
        let count = blocks.len();
        for (index, (kind, body)) in blocks.into_iter().enumerate() {
            tagged.push(if index + 1 == count { 0x80 | kind } else { kind });
            tagged.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
            tagged.extend_from_slice(body);
        }
        tagged.extend_from_slice(&data[position..]);
        Some(tagged)
    }

    fn vorbis_comment(&self) -> Vec<u8> {
        let mut comment = Vec::new();
        vorbis_string(&mut comment, b"Tactus");
        comment.extend_from_slice(&(self.fields.len() as u32).to_le_bytes());
        for (field, value) in &self.fields {
            vorbis_string(&mut comment, format!("{}={value}", field.vorbis()).as_bytes());
        }
        comment
    }
}

fn vorbis_string(comment: &mut Vec<u8>, bytes: &[u8]) {
    comment.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    comment.extend_from_slice(bytes);
}

fn mime(image: &[u8]) -> &'static str {
    if image.starts_with(b"\x89PNG") { "image/png" } else { "image/jpeg" }
}

fn id3_frame(frames: &mut Vec<u8>, id: &[u8; 4], parts: &[&[u8]]) {
    frames.extend_from_slice(id);
    frames.extend_from_slice(&syncsafe(parts.iter().map(|part| part.len()).sum()));
    frames.extend_from_slice(&[0, 0]);
    for part in parts {
        frames.extend_from_slice(part);
    }
}

fn syncsafe(length: usize) -> [u8; 4] {
    [21, 14, 7, 0].map(|shift| ((length >> shift) & 0x7F) as u8)
}

fn id3_length(data: &[u8]) -> usize {
    match data.get(..10) {
        Some(header) if header.starts_with(b"ID3") => {
            let size = header[6..].iter().fold(0, |size, byte| size << 7 | usize::from(byte & 0x7F));
            let footer = if header[5] & 0x10 == 0 { 0 } else { 10 };
            (10 + size + footer).min(data.len())
        }
        _ => 0,
    }
}

fn flac_picture(cover: &[u8]) -> Vec<u8> {
    let mime = mime(cover);
    let mut picture = Vec::with_capacity(32 + mime.len() + cover.len());
    picture.extend_from_slice(&u32::from(FRONT_COVER).to_be_bytes());
    picture.extend_from_slice(&(mime.len() as u32).to_be_bytes());
    picture.extend_from_slice(mime.as_bytes());
    picture.extend_from_slice(&[0; 20]);
    picture.extend_from_slice(&(cover.len() as u32).to_be_bytes());
    picture.extend_from_slice(cover);
    picture
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tactus_api::fake::track;

    const AUDIO: &[u8] = b"\xFF\xFB\x90\x00audio";

    fn tags(cover: Option<&[u8]>) -> Tags<'_> {
        Tags::new(&Track { year: 2001, genre: "rock".into(), ..track("1") }, Some(3), cover)
    }

    fn id3_frames(data: &[u8]) -> Vec<(String, Vec<u8>)> {
        let (mut frames, mut position, end) = (Vec::new(), 10, id3_length(data));
        while position + 10 <= end {
            let size = data[position + 4..position + 8].iter().fold(0, |size, byte| size << 7 | usize::from(*byte));
            let id = String::from_utf8_lossy(&data[position..position + 4]).into_owned();
            frames.push((id, data[position + 10..position + 10 + size].to_vec()));
            position += 10 + size;
        }
        frames
    }

    fn flac_blocks(data: &[u8]) -> Vec<(u8, bool, Vec<u8>)> {
        let (mut blocks, mut position) = (Vec::new(), 4);
        loop {
            let header = &data[position..position + 4];
            let length = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
            let last = header[0] & 0x80 != 0;
            blocks.push((header[0] & 0x7F, last, data[position + 4..position + 4 + length].to_vec()));
            position += 4 + length;
            if last {
                return blocks;
            }
        }
    }

    fn flac(audio: &[u8]) -> Vec<u8> {
        let mut data = b"fLaC\x00\x00\x00\x22".to_vec();
        data.extend_from_slice(&[7; 34]);
        data.extend_from_slice(b"\x84\x00\x00\x08");
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(b"old!");
        data.extend_from_slice(audio);
        data
    }

    #[test]
    fn writes_id3_frames_before_the_audio() {
        let tagged = tags(Some(b"\xFF\xD8jpeg")).apply(AUDIO).unwrap();
        assert!(tagged.starts_with(b"ID3\x04") && tagged.ends_with(AUDIO));
        let frames = id3_frames(&tagged);
        let ids: Vec<&str> = frames.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["TIT2", "TPE1", "TALB", "TCON", "TDRC", "TRCK", "APIC"]);
        assert_eq!(frames[0].1, b"\x03Track 1");
        assert_eq!(frames[5].1, b"\x033");
        assert_eq!(frames[6].1, b"\x03image/jpeg\x00\x03\x00\xFF\xD8jpeg");
    }

    #[test]
    fn replaces_an_existing_id3_tag() {
        let once = tags(None).apply(AUDIO).unwrap();
        assert_eq!(tags(None).apply(&once).unwrap(), once);
        let other = Tags::new(&track("2"), None, None).apply(&once).unwrap();
        assert!(other.ends_with(AUDIO));
        assert_eq!(id3_frames(&other)[0].1, b"\x03Track 2");
    }

    #[test]
    fn rebuilds_flac_metadata() {
        let tagged = tags(Some(b"\x89PNGdata")).apply(&flac(b"frames")).unwrap();
        assert!(tagged.ends_with(b"frames"));
        let blocks = flac_blocks(&tagged);
        let kinds: Vec<(u8, bool)> = blocks.iter().map(|(kind, last, _)| (*kind, *last)).collect();
        assert_eq!(kinds, [(0, false), (COMMENT, false), (PICTURE, true)]);
        assert_eq!(blocks[0].2, [7; 34]);
        let comment = String::from_utf8_lossy(&blocks[1].2).into_owned();
        assert!(
            comment.contains("TITLE=Track 1") && comment.contains("DATE=2001") && comment.contains("TRACKNUMBER=3")
        );
        assert!(!comment.contains("old!"));
        assert!(blocks[2].2.ends_with(b"\x89PNGdata"));
        assert_eq!(tags(Some(b"\x89PNGdata")).apply(&tagged).unwrap(), tagged);
    }

    #[test]
    fn rejects_unknown_and_broken_files() {
        assert_eq!(tags(None).apply(b"\x00\x00\x00\x20ftypM4A "), None);
        assert_eq!(tags(None).apply(b"fLaC\x00\x00\x00\x22short"), None);
        assert_eq!(tags(None).apply(b""), None);
    }

    proptest! {
        #[test]
        fn id3_keeps_any_title(title in "\\PC{0,200}") {
            let tags = Tags::new(&Track { title: title.clone(), ..track("1") }, None, None);
            let tagged = tags.apply(AUDIO).unwrap();
            prop_assert!(tagged.ends_with(AUDIO));
            let frames = id3_frames(&tagged);
            let expected = (!title.is_empty()).then(|| [&[UTF8], title.as_bytes()].concat());
            prop_assert_eq!(frames.iter().find(|(id, _)| id == "TIT2").map(|(_, body)| body.clone()), expected);
        }
    }
}
