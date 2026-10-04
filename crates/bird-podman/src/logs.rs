use hyper::body::Incoming;

use crate::error::check;
use crate::follow::LogFollower;
use crate::lines::Lines;
use crate::output::OutputFollower;
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
        Ok(LogFollower::new(self.follow(id, &path).await?))
    }

    // everything since the container started as written, ending when it exits
    pub async fn follow_output(&self, id: &str) -> Result<OutputFollower> {
        let path = format!(
            "/containers/{}/logs?follow=true&stdout=true&stderr=true",
            encode(id)
        );
        Ok(OutputFollower::new(self.follow(id, &path).await?))
    }

    async fn follow(&self, id: &str, path: &str) -> Result<Incoming> {
        let streamed = self.stream(path).await?;
        let status = streamed.status;
        if !status.is_success() {
            check(streamed.collect().await?, || format!("container {id}"))?;
            return Err(Error::Api {
                status: status.as_u16(),
                message: "unexpected response to a log stream".to_owned(),
            });
        }
        Ok(streamed.body)
    }
}

fn demux(body: &[u8]) -> Vec<LogLine> {
    let mut lines = Lines::default();
    if next_frame(body).is_none() {
        lines.push(LogStream::Stdout, body);
    } else {
        let mut rest = body;
        while let Some((stream, payload, next)) = next_frame(rest) {
            lines.push(stream, payload);
            rest = next;
        }
    }
    lines.finish();
    lines.ready.into()
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
    fn joins_long_lines_the_log_driver_split() {
        let mut body = frame(1, "aaa");
        body.extend(frame(1, "bbb\n"));
        body.extend(frame(1, "tail"));
        let lines = demux(&body);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "aaabbb");
        assert_eq!(lines[1].text, "tail");
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
