const PACKET: usize = 188;
const SYNC: u8 = 0x47;
const MPEG_AUDIO: [u8; 2] = [0x03, 0x04];

#[derive(Default)]
pub struct Demuxer {
    pmt: Option<u16>,
    audio: Option<u16>,
}

fn section(payload: &[u8], start: bool) -> Option<&[u8]> {
    if !start {
        return None;
    }
    let pointer = usize::from(*payload.first()?);
    let table = payload.get(1 + pointer..)?;
    let length = usize::from(u16::from_be_bytes([*table.get(1)?, *table.get(2)?]) & 0x0fff);
    table.get(..(3 + length).min(table.len()))
}

impl Demuxer {
    pub fn push(&mut self, data: &[u8], output: &mut Vec<u8>) {
        for packet in data.as_chunks::<PACKET>().0.iter().filter(|packet| packet[0] == SYNC) {
            let pid = u16::from_be_bytes([packet[1], packet[2]]) & 0x1fff;
            let start = packet[1] & 0x40 != 0;
            let control = (packet[3] >> 4) & 0x3;
            if control & 0x1 == 0 {
                continue;
            }
            let offset = if control & 0x2 != 0 { 5 + usize::from(packet[4]) } else { 4 };
            let Some(payload) = packet.get(offset..) else {
                continue;
            };
            if pid == 0 {
                self.read_pat(section(payload, start));
            } else if Some(pid) == self.pmt {
                self.read_pmt(section(payload, start));
            } else if Some(pid) == self.audio {
                output.extend_from_slice(if start { pes_payload(payload) } else { payload });
            }
        }
    }

    fn read_pat(&mut self, table: Option<&[u8]>) {
        let Some(entries) = table.and_then(|table| table.get(8..table.len().saturating_sub(4))) else {
            return;
        };
        self.pmt = entries
            .as_chunks::<4>()
            .0
            .iter()
            .find(|entry| u16::from_be_bytes([entry[0], entry[1]]) != 0)
            .map(|entry| u16::from_be_bytes([entry[2], entry[3]]) & 0x1fff);
    }

    fn read_pmt(&mut self, table: Option<&[u8]>) {
        let Some(table) = table.filter(|table| table.len() >= 12) else {
            return;
        };
        let info = usize::from(u16::from_be_bytes([table[10], table[11]]) & 0x0fff);
        let end = table.len().saturating_sub(4);
        let mut position = 12 + info;
        while position + 5 <= end {
            let kind = table[position];
            let pid = u16::from_be_bytes([table[position + 1], table[position + 2]]) & 0x1fff;
            let extra = usize::from(u16::from_be_bytes([table[position + 3], table[position + 4]]) & 0x0fff);
            if MPEG_AUDIO.contains(&kind) {
                self.audio = Some(pid);
                return;
            }
            position += 5 + extra;
        }
    }
}

fn pes_payload(payload: &[u8]) -> &[u8] {
    if payload.len() < 9 || payload[..3] != [0, 0, 1] {
        return payload;
    }
    payload.get(9 + usize::from(payload[8])..).unwrap_or_default()
}

pub fn segments(playlist: &str, base: &str) -> Vec<String> {
    let base = base.rsplit_once('/').map_or(base, |(base, _)| base);
    playlist
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| if line.contains("://") { line.to_owned() } else { format!("{base}/{line}") })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(pid: u16, start: bool, payload: &[u8]) -> Vec<u8> {
        let mut packet = vec![SYNC, ((pid >> 8) as u8) | if start { 0x40 } else { 0 }, pid as u8];
        let stuffing = PACKET - 4 - payload.len();
        if stuffing == 0 {
            packet.push(0x10);
        } else {
            packet.push(0x30);
            packet.push((stuffing - 1) as u8);
            if stuffing > 1 {
                packet.push(0);
                packet.extend(std::iter::repeat_n(0xff, stuffing - 2));
            }
        }
        packet.extend_from_slice(payload);
        packet
    }

    fn table(id: u8, body: &[u8]) -> Vec<u8> {
        let length = (body.len() + 5 + 4) as u16;
        let mut table = vec![0, id, 0xb0 | (length >> 8) as u8, length as u8, 0, 1, 0xc1, 0, 0];
        table.extend_from_slice(body);
        table.extend_from_slice(&[0; 4]);
        table
    }

    fn stream() -> Vec<u8> {
        let pat = table(0, &[0, 1, 0xef, 0xff]);
        let pmt = table(2, &[0xe1, 0x00, 0xf0, 0x00, 0x15, 0xe1, 0x01, 0xf0, 0x00, 0x03, 0xe1, 0x00, 0xf0, 0x00]);
        let mut data = packet(0, true, &pat);
        data.extend(packet(0x0fff, true, &pmt));
        data.extend(packet(0x101, true, b"\0\0\x01\xbd\0\0\x80\0\0id3"));
        data.extend(packet(0x100, true, &[b"\0\0\x01\xc0\0\0\x80\x80\x05aaaaa".as_slice(), b"MP3-1"].concat()));
        data.extend(packet(0x100, false, b"MP3-2"));
        data
    }

    #[test]
    fn extracts_mpeg_audio_from_transport_stream() {
        let mut demuxer = Demuxer::default();
        let mut output = Vec::new();
        demuxer.push(&stream(), &mut output);
        assert_eq!(output, b"MP3-1MP3-2");
        let mut more = Vec::new();
        demuxer.push(&packet(0x100, true, b"\0\0\x01\xc0\0\0\x80\0\0MP3-3"), &mut more);
        assert_eq!(more, b"MP3-3");
    }

    #[test]
    fn ignores_garbage() {
        let mut demuxer = Demuxer::default();
        let mut output = Vec::new();
        demuxer.push(&[0u8; PACKET * 2], &mut output);
        demuxer.push(&[SYNC, 0, 0], &mut output);
        assert!(output.is_empty());
    }

    #[test]
    fn lists_playlist_segments() {
        let playlist =
            "#EXTM3U\n#EXTINF:4.9,static\n00001.ts\n\n#EXTINF:4.9\nhttps://cdn.example/x.ts\n#EXT-X-ENDLIST\n";
        assert_eq!(
            segments(playlist, "https://host/stream/00000.m3u8"),
            ["https://host/stream/00001.ts", "https://cdn.example/x.ts"]
        );
    }
}
