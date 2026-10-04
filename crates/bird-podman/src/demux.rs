use crate::logs::{FRAME_HEADER_LEN, LogStream};
use crate::{Error, Result};

// splits podman's stdout and stderr frames as bytes arrive, passing on parts of a frame instead
// of holding a whole one, so frames of any size go through in bounded memory
#[derive(Debug, Default)]
pub struct Demux {
    header: Vec<u8>,
    current: Option<(LogStream, usize)>,
}

impl Demux {
    // the output in `data`, in order; a frame cut off at its end continues in the next call
    pub fn push<'a>(&mut self, mut data: &'a [u8]) -> Result<Vec<(LogStream, &'a [u8])>> {
        let mut pieces = Vec::new();
        while !data.is_empty() {
            if let Some((stream, remaining)) = self.current {
                let (piece, rest) = data.split_at(remaining.min(data.len()));
                pieces.push((stream, piece));
                data = rest;
                let left = remaining - piece.len();
                self.current = (left > 0).then_some((stream, left));
                continue;
            }
            let wanted = FRAME_HEADER_LEN - self.header.len();
            let (part, rest) = data.split_at(wanted.min(data.len()));
            self.header.extend_from_slice(part);
            data = rest;
            if self.header.len() == FRAME_HEADER_LEN {
                let header = std::mem::take(&mut self.header);
                self.current = start(&header)?;
            }
        }
        Ok(pieces)
    }
}

fn start(header: &[u8]) -> Result<Option<(LogStream, usize)>> {
    let &[kind, 0, 0, 0, a, b, c, d] = header else {
        return Err(unframed());
    };
    let stream = match kind {
        0 | 1 => LogStream::Stdout,
        2 => LogStream::Stderr,
        _ => return Err(unframed()),
    };
    let len = usize::try_from(u32::from_be_bytes([a, b, c, d]))
        .map_err(|_| Error::Body("output frame length does not fit".into()))?;
    Ok((len > 0).then_some((stream, len)))
}

fn unframed() -> Error {
    Error::Body("command output is not in podman's stream format".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: u8, bytes: &[u8]) -> Vec<u8> {
        let len = u32::try_from(bytes.len()).unwrap().to_be_bytes();
        let mut out = vec![kind, 0, 0, 0];
        out.extend_from_slice(&len);
        out.extend_from_slice(bytes);
        out
    }

    fn collect(demux: &mut Demux, data: &[u8], into: &mut Vec<(LogStream, Vec<u8>)>) {
        for (stream, piece) in demux.push(data).unwrap() {
            match into.last_mut() {
                Some((last, bytes)) if *last == stream => bytes.extend_from_slice(piece),
                _ => into.push((stream, piece.to_vec())),
            }
        }
    }

    #[test]
    fn splits_frames_fed_a_byte_at_a_time() {
        let mut data = frame(1, &[0, 0xff, b'\n', 0x80]);
        data.extend(frame(2, b"oops"));
        data.extend(frame(1, b""));
        data.extend(frame(1, b"end"));
        let mut demux = Demux::default();
        let mut out = Vec::new();
        for byte in &data {
            collect(&mut demux, std::slice::from_ref(byte), &mut out);
        }
        assert_eq!(
            out,
            vec![
                (LogStream::Stdout, vec![0, 0xff, b'\n', 0x80]),
                (LogStream::Stderr, b"oops".to_vec()),
                (LogStream::Stdout, b"end".to_vec()),
            ]
        );
    }

    #[test]
    fn passes_on_parts_of_a_large_frame() {
        let mut demux = Demux::default();
        let mut data = vec![1, 0, 0, 0];
        data.extend_from_slice(&u32::MAX.to_be_bytes());
        data.extend_from_slice(b"partial");
        assert_eq!(
            demux.push(&data).unwrap(),
            vec![(LogStream::Stdout, &b"partial"[..])]
        );
        assert_eq!(
            demux.push(b"more").unwrap(),
            vec![(LogStream::Stdout, &b"more"[..])]
        );
    }

    #[test]
    fn rejects_unframed_output() {
        assert!(Demux::default().push(b"plain text here").is_err());
        assert!(Demux::default().push(&frame(3, b"x")).is_err());
    }
}
