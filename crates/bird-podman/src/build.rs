use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use bird_core::{EnvKey, ImageRef};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use serde::Deserialize;

use crate::error::check;
use crate::query::encode;
use crate::{Error, Podman, Result};

// covers uploading the context; the build itself streams for as long as it takes
const UPLOAD_TIMEOUT: Duration = Duration::from_mins(10);
const MAX_LINE_BYTES: usize = 1024 * 1024;
// podman's build api stores layers but only reuses them when the output format is named
// (podman 5.8), so leaving it out silently rebuilds every step
const OUTPUT_FORMAT: &str = "application/vnd.oci.image.manifest.v1+json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildLine {
    Log(String),
    Failed(String),
}

#[derive(Deserialize)]
struct BuildReport {
    #[serde(default)]
    stream: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

pub struct BuildOutput {
    body: Incoming,
    buffer: Vec<u8>,
    pending: VecDeque<BuildLine>,
}

impl Podman {
    // context is a tar archive, optionally gzipped; podman applies .dockerignore itself
    pub async fn build_image<B>(
        &self,
        context: B,
        tag: &ImageRef,
        dockerfile: &str,
        args: &BTreeMap<EnvKey, String>,
    ) -> Result<BuildOutput>
    where
        B: hyper::body::Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let mut path = format!(
            "/build?t={}&dockerfile={}&rm=true&layers=true&outputformat={}",
            encode(&tag.qualified()),
            encode(dockerfile),
            encode(OUTPUT_FORMAT)
        );
        if !args.is_empty() {
            path.push_str("&buildargs=");
            path.push_str(&encode(&serde_json::to_string(args)?));
        }
        let streamed = self
            .upload(&path, "application/x-tar", context, UPLOAD_TIMEOUT)
            .await?;
        if !streamed.status.is_success() {
            let status = streamed.status.as_u16();
            check(streamed.collect().await?, || format!("build of {tag}"))?;
            return Err(Error::Api {
                status,
                message: "unexpected response to a build".to_owned(),
            });
        }
        Ok(BuildOutput {
            body: streamed.body,
            buffer: Vec::new(),
            pending: VecDeque::new(),
        })
    }
}

impl BuildOutput {
    pub async fn next(&mut self) -> Option<Result<BuildLine>> {
        loop {
            if let Some(line) = self.pending.pop_front() {
                return Some(Ok(line));
            }
            match self.body.frame().await {
                Some(Ok(frame)) => {
                    if let Ok(data) = frame.into_data() {
                        self.buffer.extend_from_slice(&data);
                        drain(&mut self.buffer, &mut self.pending);
                    }
                }
                Some(Err(err)) => return Some(Err(Error::Http(err))),
                None => {
                    let rest = std::mem::take(&mut self.buffer);
                    parse_line(&rest, &mut self.pending);
                    return self.pending.pop_front().map(Ok);
                }
            }
        }
    }
}

fn drain(buffer: &mut Vec<u8>, pending: &mut VecDeque<BuildLine>) {
    while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
        let line: Vec<u8> = buffer.drain(..=end).collect();
        parse_line(&line, pending);
    }
    if buffer.len() > MAX_LINE_BYTES {
        let line = std::mem::take(buffer);
        parse_line(&line, pending);
    }
}

fn parse_line(line: &[u8], pending: &mut VecDeque<BuildLine>) {
    let line = line.trim_ascii();
    if line.is_empty() {
        return;
    }
    match serde_json::from_slice::<BuildReport>(line) {
        Ok(BuildReport {
            error: Some(error), ..
        }) => pending.push_back(BuildLine::Failed(error.trim_end().to_owned())),
        Ok(BuildReport {
            stream: Some(text), ..
        }) => pending.extend(text.lines().map(|l| BuildLine::Log(l.to_owned()))),
        Ok(_) => {}
        Err(_) => pending.push_back(BuildLine::Log(String::from_utf8_lossy(line).into_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_and_errors() {
        let mut buffer = br#"{"stream":"STEP 1/2: FROM alpine\n"}
{"stream":"boom\n"}
{"errorDetail":{"message":"exit status 7\n"},"error":"exit status 7\n"}
{"stream":"partial"#
            .to_vec();
        let mut pending = VecDeque::new();
        drain(&mut buffer, &mut pending);
        assert_eq!(
            Vec::from(pending),
            vec![
                BuildLine::Log("STEP 1/2: FROM alpine".to_owned()),
                BuildLine::Log("boom".to_owned()),
                BuildLine::Failed("exit status 7".to_owned()),
            ]
        );
        assert_eq!(buffer, br#"{"stream":"partial"#);
    }
}
