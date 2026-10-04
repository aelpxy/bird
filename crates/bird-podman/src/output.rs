use std::collections::VecDeque;

use http_body_util::BodyExt;
use hyper::body::Incoming;

use crate::logs::{FRAME_HEADER_LEN, LogStream, next_frame};
use crate::{Error, Result};

const MAX_FRAME_BYTES: usize = 1024 * 1024;

// a piece of a command's output exactly as written, newlines included; never splits a character
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChunk {
    pub stream: LogStream,
    pub text: String,
}

// a command's stdout and stderr as written, unlike LogFollower which splits them into lines
pub struct OutputFollower {
    body: Incoming,
    decoder: Decoder,
}

impl OutputFollower {
    pub(crate) fn new(body: Incoming) -> Self {
        Self {
            body,
            decoder: Decoder::default(),
        }
    }

    pub async fn next(&mut self) -> Option<Result<OutputChunk>> {
        loop {
            if let Some(chunk) = self.decoder.pending.pop_front() {
                return Some(Ok(chunk));
            }
            let Some(frame) = self.body.frame().await else {
                self.decoder.finish();
                return self.decoder.pending.pop_front().map(Ok);
            };
            match frame {
                Ok(frame) => {
                    if let Ok(data) = frame.into_data()
                        && let Err(err) = self.decoder.push(&data)
                    {
                        return Some(Err(err));
                    }
                }
                Err(err) => return Some(Err(Error::Http(err))),
            }
        }
    }
}

#[derive(Default)]
struct Decoder {
    buffer: Vec<u8>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    pending: VecDeque<OutputChunk>,
}

impl Decoder {
    fn push(&mut self, data: &[u8]) -> Result<()> {
        self.buffer.extend_from_slice(data);
        let buffer = std::mem::take(&mut self.buffer);
        let mut rest = buffer.as_slice();
        while let Some((stream, payload, next)) = next_frame(rest) {
            self.decode(stream, payload, false);
            rest = next;
        }
        self.buffer = rest.to_vec();
        if self.buffer.len() >= FRAME_HEADER_LEN && !header_is_valid(&self.buffer) {
            return Err(Error::Body(
                "command output is not in podman's stream format".into(),
            ));
        }
        if self.buffer.len() > MAX_FRAME_BYTES + FRAME_HEADER_LEN {
            return Err(Error::Body("command output frame is too large".into()));
        }
        Ok(())
    }

    // a character cut off at the end of the output is shown as a replacement character
    fn finish(&mut self) {
        self.decode(LogStream::Stdout, &[], true);
        self.decode(LogStream::Stderr, &[], true);
    }

    fn decode(&mut self, stream: LogStream, payload: &[u8], last: bool) {
        let carry = match stream {
            LogStream::Stdout => &mut self.stdout,
            LogStream::Stderr => &mut self.stderr,
        };
        carry.extend_from_slice(payload);
        let keep = if last { 0 } else { incomplete_tail(carry) };
        let complete = carry.len() - keep;
        if complete == 0 {
            return;
        }
        let text = String::from_utf8_lossy(carry.get(..complete).unwrap_or_default()).into_owned();
        carry.drain(..complete);
        self.pending.push_back(OutputChunk { stream, text });
    }
}

fn header_is_valid(buffer: &[u8]) -> bool {
    matches!(buffer, [0..=2, 0, 0, 0, ..])
}

// bytes at the end that start a character the next frame finishes
fn incomplete_tail(bytes: &[u8]) -> usize {
    for back in 1..=bytes.len().min(4) {
        let Some(&byte) = bytes.get(bytes.len() - back) else {
            return 0;
        };
        if byte & 0b1100_0000 == 0b1000_0000 {
            continue;
        }
        let length = match byte {
            0xF0.. => 4,
            0xE0.. => 3,
            0xC0.. => 2,
            _ => 1,
        };
        return if length > back { back } else { 0 };
    }
    0
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

    fn chunk(stream: LogStream, text: &str) -> OutputChunk {
        OutputChunk {
            stream,
            text: text.to_owned(),
        }
    }

    #[test]
    fn keeps_output_as_written() {
        let mut decoder = Decoder::default();
        let mut data = frame(1, b"half a li");
        data.extend(frame(1, b"ne\nno newline"));
        data.extend(frame(2, b"oops"));
        decoder.push(&data).unwrap();
        decoder.finish();
        assert_eq!(
            Vec::from(decoder.pending),
            vec![
                chunk(LogStream::Stdout, "half a li"),
                chunk(LogStream::Stdout, "ne\nno newline"),
                chunk(LogStream::Stderr, "oops"),
            ]
        );
    }

    #[test]
    fn joins_characters_split_across_frames() {
        let text = "é€😀";
        let bytes = text.as_bytes();
        let mut decoder = Decoder::default();
        for (i, byte) in bytes.iter().enumerate() {
            let mut data = frame(1, &[*byte]);
            if i == 0 {
                data.extend(frame(2, b"x"));
            }
            decoder.push(&data).unwrap();
        }
        let stdout: String = decoder
            .pending
            .iter()
            .filter(|c| c.stream == LogStream::Stdout)
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(stdout, text);
        assert_eq!(decoder.stdout, Vec::<u8>::new());
    }

    #[test]
    fn waits_for_split_frames_and_flags_cut_characters() {
        let whole = frame(1, "a€".as_bytes());
        let (first, second) = whole.split_at(5);
        let mut decoder = Decoder::default();
        decoder.push(first).unwrap();
        assert!(decoder.pending.is_empty());
        decoder.push(second).unwrap();
        decoder.push(&frame(1, &"€".as_bytes()[..2])).unwrap();
        decoder.finish();
        assert_eq!(decoder.pending[0], chunk(LogStream::Stdout, "a€"));
        assert_eq!(decoder.pending[1], chunk(LogStream::Stdout, "\u{FFFD}"));
    }

    #[test]
    fn rejects_unframed_or_oversized_output() {
        assert!(Decoder::default().push(b"plain text here").is_err());
        let mut huge = vec![1, 0, 0, 0];
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        huge.resize(MAX_FRAME_BYTES + 100, 0);
        assert!(Decoder::default().push(&huge).is_err());
    }

    #[test]
    fn finds_incomplete_characters() {
        assert_eq!(incomplete_tail(b"abc"), 0);
        assert_eq!(incomplete_tail(&"é".as_bytes()[..1]), 1);
        assert_eq!(incomplete_tail(&"😀".as_bytes()[..3]), 3);
        assert_eq!(incomplete_tail("😀".as_bytes()), 0);
        assert_eq!(incomplete_tail(&[0x80, 0x80, 0x80, 0x80]), 0);
    }
}
