use crate::logs::{FRAME_HEADER_LEN, LogStream};
use crate::{Error, Result};

// splits podman's stdout and stderr frames as bytes arrive, passing on parts of a frame instead
// of holding a whole one, so frames of any size go through in bounded memory
#[derive(Debug, Default)]
pub struct Demux {
    header: Vec<u8>,
    current: Option<Current>,
}

#[derive(Debug, Clone, Copy)]
struct Current {
    stream: LogStream,
    len: usize,
    remaining: usize,
}

// part of one frame; a frame arrives as one or more pieces, from `starts` to `ends`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece<'a> {
    pub stream: LogStream,
    pub data: &'a [u8],
    // the whole frame's length, known from its header
    pub frame_len: usize,
    pub starts: bool,
    pub ends: bool,
}

impl Demux {
    // the output in `data`, in order; a frame cut off at its end continues in the next call
    pub fn push<'a>(&mut self, mut data: &'a [u8]) -> Result<Vec<Piece<'a>>> {
        let mut pieces = Vec::new();
        while !data.is_empty() {
            if let Some(current) = self.current {
                let (piece, rest) = data.split_at(current.remaining.min(data.len()));
                let left = current.remaining - piece.len();
                pieces.push(Piece {
                    stream: current.stream,
                    data: piece,
                    frame_len: current.len,
                    starts: current.remaining == current.len,
                    ends: left == 0,
                });
                data = rest;
                self.current = (left > 0).then_some(Current {
                    remaining: left,
                    ..current
                });
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

// what podman itself writes to stderr when a command exits while its input is still being sent,
// like `Error: write unixpacket @->/proc/self/fd/15/attach: write: connection reset by peer`; it
// is not the command's output
#[must_use]
pub fn is_attach_reset_notice(frame: &[u8]) -> bool {
    let contains = |needle: &[u8]| frame.windows(needle.len()).any(|window| window == needle);
    frame.starts_with(b"Error: ")
        && contains(b" unixpacket ")
        && contains(b"/attach: ")
        && frame.ends_with(b": connection reset by peer\n")
}

fn start(header: &[u8]) -> Result<Option<Current>> {
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
    Ok((len > 0).then_some(Current {
        stream,
        len,
        remaining: len,
    }))
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
        for piece in demux.push(data).unwrap() {
            match into.last_mut() {
                Some((last, bytes)) if *last == piece.stream && !piece.starts => {
                    bytes.extend_from_slice(piece.data);
                }
                _ => into.push((piece.stream, piece.data.to_vec())),
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
    fn marks_where_frames_start_and_end() {
        let mut data = frame(2, b"abcdef");
        data.extend(frame(1, b"x"));
        let mut demux = Demux::default();
        let first = demux.push(&data[..11]).unwrap();
        assert_eq!(
            first,
            [Piece {
                stream: LogStream::Stderr,
                data: b"abc",
                frame_len: 6,
                starts: true,
                ends: false,
            }]
        );
        let rest = demux.push(&data[11..]).unwrap();
        assert_eq!(
            (rest[0].data, rest[0].starts, rest[0].ends),
            (&b"def"[..], false, true)
        );
        assert_eq!(
            (rest[1].stream, rest[1].starts, rest[1].ends),
            (LogStream::Stdout, true, true)
        );
    }

    #[test]
    fn passes_on_parts_of_a_large_frame() {
        let mut demux = Demux::default();
        let mut data = vec![1, 0, 0, 0];
        data.extend_from_slice(&u32::MAX.to_be_bytes());
        data.extend_from_slice(b"partial");
        assert_eq!(demux.push(&data).unwrap()[0].data, b"partial");
        assert_eq!(demux.push(b"more").unwrap()[0].data, b"more");
    }

    #[test]
    fn rejects_unframed_output() {
        assert!(Demux::default().push(b"plain text here").is_err());
        assert!(Demux::default().push(&frame(3, b"x")).is_err());
    }

    #[test]
    fn recognizes_podmans_attach_reset_notice() {
        assert!(is_attach_reset_notice(
            b"Error: write unixpacket @->/proc/self/fd/15/attach: write: connection reset by peer\n"
        ));
        assert!(is_attach_reset_notice(
            b"Error: read unixpacket @->/proc/self/fd/9/attach: read: connection reset by peer\n"
        ));
        assert!(!is_attach_reset_notice(
            b"Error: connection reset by peer\n"
        ));
        assert!(!is_attach_reset_notice(
            b"psql: error: connection reset by peer\n"
        ));
    }
}
