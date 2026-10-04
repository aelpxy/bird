use bird_api::Frame;
use bird_podman::{Demux, LogStream, is_attach_reset_notice};

// stderr frames up to this size are passed on whole, which lets podman's notice be recognized
const MAX_HELD_BYTES: usize = 512;

// turns podman's stdout and stderr frames into bird-tty frames as bytes arrive; podman's own
// notice that the command exited with input unsent is held back and dropped if nothing follows
#[derive(Debug, Default)]
pub(crate) struct PipedOutput {
    demux: Demux,
    held: Vec<u8>,
    notice: Option<Vec<u8>>,
}

impl PipedOutput {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> bird_podman::Result<Vec<Frame>> {
        let mut frames = Vec::new();
        for piece in self.demux.push(bytes)? {
            // more output after it means it was the command's own line after all
            if let Some(notice) = self.notice.take() {
                frames.push(Frame::Stderr(notice));
            }
            match piece.stream {
                LogStream::Stdout => frames.push(Frame::Data(piece.data.to_vec())),
                LogStream::Stderr if piece.frame_len <= MAX_HELD_BYTES => {
                    self.held.extend_from_slice(piece.data);
                    if piece.ends {
                        let frame = std::mem::take(&mut self.held);
                        if is_attach_reset_notice(&frame) {
                            self.notice = Some(frame);
                        } else {
                            frames.push(Frame::Stderr(frame));
                        }
                    }
                }
                LogStream::Stderr => frames.push(Frame::Stderr(piece.data.to_vec())),
            }
        }
        Ok(frames)
    }

    // what is left once the output ends: part of a frame cut short, but never podman's notice
    pub(crate) fn finish(&mut self) -> Option<Frame> {
        if self.notice.take().is_some() {
            tracing::debug!("dropped podman's notice about input the command did not read");
        }
        let held = std::mem::take(&mut self.held);
        (!held.is_empty()).then_some(Frame::Stderr(held))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTICE: &[u8] =
        b"Error: write unixpacket @->/proc/self/fd/15/attach: write: connection reset by peer\n";

    fn frame(kind: u8, bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![kind, 0, 0, 0];
        out.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_be_bytes());
        out.extend_from_slice(bytes);
        out
    }

    fn run(chunks: &[&[u8]]) -> Vec<Frame> {
        let mut output = PipedOutput::default();
        let mut frames: Vec<Frame> = chunks
            .iter()
            .flat_map(|chunk| output.push(chunk).unwrap())
            .collect();
        frames.extend(output.finish());
        frames
    }

    #[test]
    fn drops_podmans_notice_when_it_comes_last() {
        let mut data = frame(1, b"done\n");
        data.extend(frame(2, NOTICE));
        assert_eq!(run(&[&data]), [Frame::Data(b"done\n".to_vec())]);
    }

    #[test]
    fn keeps_the_same_line_when_more_output_follows() {
        let mut data = frame(2, NOTICE);
        data.extend(frame(1, b"after"));
        assert_eq!(
            run(&[&data]),
            [
                Frame::Stderr(NOTICE.to_vec()),
                Frame::Data(b"after".to_vec())
            ]
        );
    }

    #[test]
    fn stderr_arrives_whole_however_it_is_split() {
        let data = [frame(2, b"warning: one"), frame(2, NOTICE)].concat();
        let chunks: Vec<&[u8]> = data.chunks(3).collect();
        assert_eq!(run(&chunks), [Frame::Stderr(b"warning: one".to_vec())]);
        let big = vec![b'e'; MAX_HELD_BYTES + 1];
        let frames = run(&[&frame(2, &big)]);
        assert_eq!(frames, [Frame::Stderr(big)]);
    }
}
