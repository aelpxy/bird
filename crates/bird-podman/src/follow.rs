use std::collections::VecDeque;

use http_body_util::BodyExt;
use hyper::body::Incoming;

use crate::logs::{FRAME_HEADER_LEN, LogLine, LogStream, next_frame, push_lines};
use crate::{Error, Result};

const MAX_BUFFERED_BYTES: usize = 1024 * 1024;

pub struct LogFollower {
    body: Incoming,
    buffer: Vec<u8>,
    pending: VecDeque<LogLine>,
}

impl LogFollower {
    pub(crate) fn new(body: Incoming) -> Self {
        Self {
            body,
            buffer: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    pub async fn next(&mut self) -> Option<Result<LogLine>> {
        loop {
            if let Some(line) = self.pending.pop_front() {
                return Some(Ok(line));
            }
            match self.body.frame().await? {
                Ok(frame) => {
                    if let Ok(data) = frame.into_data() {
                        self.buffer.extend_from_slice(&data);
                        drain(&mut self.buffer, &mut self.pending);
                    }
                }
                Err(err) => return Some(Err(Error::Http(err))),
            }
        }
    }
}

fn drain(buffer: &mut Vec<u8>, pending: &mut VecDeque<LogLine>) {
    let mut rest = buffer.as_slice();
    while let Some((stream, payload, next)) = next_frame(rest) {
        push_lines(pending, stream, payload);
        rest = next;
    }
    let consumed = buffer.len() - rest.len();
    buffer.drain(..consumed);

    // unframed output (a tty container) or a runaway frame is passed through as plain text
    let unframed = buffer.len() >= FRAME_HEADER_LEN && !header_is_valid(buffer);
    if unframed || buffer.len() > MAX_BUFFERED_BYTES {
        push_lines(pending, LogStream::Stdout, buffer);
        buffer.clear();
    }
}

fn header_is_valid(buffer: &[u8]) -> bool {
    matches!(buffer, [0..=2, 0, 0, 0, ..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: u8, text: &str) -> Vec<u8> {
        let len = u32::try_from(text.len()).unwrap().to_be_bytes();
        let mut out = vec![kind, 0, 0, 0];
        out.extend_from_slice(&len);
        out.extend_from_slice(text.as_bytes());
        out
    }

    #[test]
    fn waits_for_split_frames() {
        let whole = frame(1, "hello\n");
        let (first, second) = whole.split_at(5);
        let mut buffer = first.to_vec();
        let mut pending = VecDeque::new();
        drain(&mut buffer, &mut pending);
        assert!(pending.is_empty());
        buffer.extend_from_slice(second);
        drain(&mut buffer, &mut pending);
        assert_eq!(pending.pop_front().unwrap().text, "hello");
        assert_eq!(buffer, Vec::<u8>::new());
    }

    #[test]
    fn keeps_partial_trailing_frame() {
        let mut buffer = frame(2, "oops\n");
        buffer.extend_from_slice(&frame(1, "next\n")[..6]);
        let mut pending = VecDeque::new();
        drain(&mut buffer, &mut pending);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].stream, LogStream::Stderr);
        assert_eq!(buffer.len(), 6);
    }

    #[test]
    fn passes_unframed_output_through() {
        let mut buffer = b"plain tty output\n".to_vec();
        let mut pending = VecDeque::new();
        drain(&mut buffer, &mut pending);
        assert_eq!(pending[0].text, "plain tty output");
        assert_eq!(buffer, Vec::<u8>::new());
    }
}
