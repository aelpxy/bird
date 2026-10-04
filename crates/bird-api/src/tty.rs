use std::fmt;

// the protocol name in the Upgrade header of `GET /v1/services/{name}/exec/tty`
pub const TTY_UPGRADE: &str = "bird-tty";

const HEADER_LEN: usize = 5;
// a terminal moves small chunks, a larger frame means a broken or hostile peer
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

const DATA: u8 = 0;
const RESIZE: u8 = 1;
const EXIT: u8 = 2;
const ERROR: u8 = 3;
const STDERR: u8 = 4;
const EOF: u8 = 5;

// one message on an upgraded connection: a kind byte, a big-endian u32 length, the payload.
// The client sends data (keystrokes or stdin), resizes and eof; birdd sends data (output, stdout
// when piped) and stderr (only when piped), then exit or error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Data(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Exit(i32),
    Error(String),
    Stderr(Vec<u8>),
    // the client's stdin ended, so the command reads end of file
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameError(String);

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid terminal frame: {}", self.0)
    }
}

impl std::error::Error for FrameError {}

impl Frame {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let (kind, payload): (u8, std::borrow::Cow<'_, [u8]>) = match self {
            Self::Data(bytes) => (DATA, bytes.as_slice().into()),
            Self::Resize { cols, rows } => {
                let mut size = cols.to_be_bytes().to_vec();
                size.extend_from_slice(&rows.to_be_bytes());
                (RESIZE, size.into())
            }
            Self::Exit(code) => (EXIT, code.to_be_bytes().to_vec().into()),
            Self::Error(message) => (ERROR, message.as_bytes().into()),
            Self::Stderr(bytes) => (STDERR, bytes.as_slice().into()),
            Self::Eof => (EOF, Vec::new().into()),
        };
        let len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
        let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
        out.push(kind);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&payload);
        out
    }

    // takes one complete frame off the front of `buffer`; None until enough bytes arrived
    pub fn decode(buffer: &mut Vec<u8>) -> Result<Option<Self>, FrameError> {
        let Some((&[kind, a, b, c, d], rest)) = buffer.split_first_chunk::<HEADER_LEN>() else {
            return Ok(None);
        };
        let len = usize::try_from(u32::from_be_bytes([a, b, c, d]))
            .map_err(|_| FrameError("length does not fit".to_owned()))?;
        if len > MAX_FRAME_BYTES {
            return Err(FrameError(format!("{len} bytes is too large")));
        }
        let Some(payload) = rest.get(..len) else {
            return Ok(None);
        };
        let frame = match (kind, payload) {
            (DATA, bytes) => Self::Data(bytes.to_vec()),
            (RESIZE, &[c1, c2, r1, r2]) => Self::Resize {
                cols: u16::from_be_bytes([c1, c2]),
                rows: u16::from_be_bytes([r1, r2]),
            },
            (EXIT, &[a, b, c, d]) => Self::Exit(i32::from_be_bytes([a, b, c, d])),
            (ERROR, bytes) => Self::Error(String::from_utf8_lossy(bytes).into_owned()),
            (STDERR, bytes) => Self::Stderr(bytes.to_vec()),
            (EOF, []) => Self::Eof,
            (kind, _) => return Err(FrameError(format!("kind {kind} with {len} bytes"))),
        };
        buffer.drain(..HEADER_LEN + len);
        Ok(Some(frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let frames = [
            Frame::Data(b"ls -la\r".to_vec()),
            Frame::Data(Vec::new()),
            Frame::Resize {
                cols: 132,
                rows: 40,
            },
            Frame::Exit(-1),
            Frame::Error("machine is gone".to_owned()),
            Frame::Stderr(vec![0, 0xff]),
            Frame::Eof,
        ];
        let mut buffer: Vec<u8> = frames.iter().flat_map(Frame::encode).collect();
        for frame in frames {
            assert_eq!(Frame::decode(&mut buffer).unwrap(), Some(frame));
        }
        assert_eq!(Frame::decode(&mut buffer).unwrap(), None);
        assert_eq!(buffer, Vec::<u8>::new());
    }

    #[test]
    fn waits_for_split_frames() {
        let whole = Frame::Data(b"hello".to_vec()).encode();
        let (first, second) = whole.split_at(7);
        let mut buffer = first.to_vec();
        assert_eq!(Frame::decode(&mut buffer).unwrap(), None);
        buffer.extend_from_slice(second);
        assert_eq!(
            Frame::decode(&mut buffer).unwrap(),
            Some(Frame::Data(b"hello".to_vec()))
        );
    }

    #[test]
    fn rejects_unknown_oversized_or_malformed_frames() {
        let mut unknown = vec![9, 0, 0, 0, 0];
        assert!(Frame::decode(&mut unknown).is_err());
        let mut huge = vec![DATA, 0x7f, 0xff, 0xff, 0xff];
        assert!(Frame::decode(&mut huge).is_err());
        let mut short_resize = vec![RESIZE, 0, 0, 0, 2, 0, 80];
        assert!(Frame::decode(&mut short_resize).is_err());
        let mut eof_with_data = vec![EOF, 0, 0, 0, 1, 0];
        assert!(Frame::decode(&mut eof_with_data).is_err());
    }
}
