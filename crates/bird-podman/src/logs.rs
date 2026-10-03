use crate::error::check;
use crate::follow::LogFollower;
use crate::query::encode;
use crate::{Error, Podman, Result};

pub(crate) const FRAME_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub stream: LogStream,
    pub text: String,
}

impl Podman {
    pub async fn logs(&self, id: &str, tail: u32) -> Result<Vec<LogLine>> {
        let path = format!(
            "/containers/{}/logs?stdout=true&stderr=true&tail={tail}",
            encode(id)
        );
        let response = self.get(&path).await?;
        let body = check(response, || format!("container {id}"))?;
        Ok(demux(&body))
    }

    pub async fn follow_logs(&self, id: &str, tail: u32) -> Result<LogFollower> {
        let path = format!(
            "/containers/{}/logs?follow=true&stdout=true&stderr=true&tail={tail}",
            encode(id)
        );
        let streamed = self.stream(&path).await?;
        let status = streamed.status;
        if !status.is_success() {
            check(streamed.collect().await?, || format!("container {id}"))?;
            return Err(Error::Api {
                status: status.as_u16(),
                message: "unexpected response to a log stream".to_owned(),
            });
        }
        Ok(LogFollower::new(streamed.body))
    }
}

fn demux(body: &[u8]) -> Vec<LogLine> {
    let mut lines = Vec::new();
    if next_frame(body).is_none() {
        push_lines(&mut lines, LogStream::Stdout, body);
        return lines;
    }
    let mut rest = body;
    while let Some((stream, payload, next)) = next_frame(rest) {
        push_lines(&mut lines, stream, payload);
        rest = next;
    }
    lines
}

pub(crate) fn next_frame(buf: &[u8]) -> Option<(LogStream, &[u8], &[u8])> {
    let (header, body) = buf.split_first_chunk::<FRAME_HEADER_LEN>()?;
    let [kind, 0, 0, 0, a, b, c, d] = *header else {
        return None;
    };
    let stream = match kind {
        0 | 1 => LogStream::Stdout,
        2 => LogStream::Stderr,
        _ => return None,
    };
    let len = usize::try_from(u32::from_be_bytes([a, b, c, d])).ok()?;
    let (payload, next) = body.split_at_checked(len)?;
    Some((stream, payload, next))
}

pub(crate) fn push_lines(lines: &mut impl Extend<LogLine>, stream: LogStream, payload: &[u8]) {
    lines.extend(
        String::from_utf8_lossy(payload)
            .lines()
            .map(|text| LogLine {
                stream,
                text: text.to_owned(),
            }),
    );
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
    fn demuxes_frames() {
        let mut body = frame(1, "hello\n");
        body.extend(frame(2, "oops\nagain\n"));
        assert_eq!(
            demux(&body),
            vec![
                LogLine {
                    stream: LogStream::Stdout,
                    text: "hello".to_owned()
                },
                LogLine {
                    stream: LogStream::Stderr,
                    text: "oops".to_owned()
                },
                LogLine {
                    stream: LogStream::Stderr,
                    text: "again".to_owned()
                },
            ]
        );
    }

    #[test]
    fn falls_back_to_raw_text() {
        let lines = demux(b"plain line\nsecond\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "plain line");
    }

    #[test]
    fn drops_truncated_trailing_frame() {
        let mut body = frame(1, "ok\n");
        body.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 99, b'x']);
        let lines = demux(&body);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "ok");
    }
}
