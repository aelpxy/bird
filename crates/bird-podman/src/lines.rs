use std::collections::VecDeque;

use crate::logs::{LogLine, LogStream};

// a line longer than this is passed on in pieces rather than held in memory
const MAX_LINE_BYTES: usize = 1024 * 1024;

// podman's log driver stores a long line as several entries and returns each as a frame without a
// trailing newline, so frames are joined back until one ends the line
#[derive(Default)]
pub(crate) struct Lines {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    pub(crate) ready: VecDeque<LogLine>,
}

impl Lines {
    pub(crate) fn push(&mut self, stream: LogStream, payload: &[u8]) {
        let partial = match stream {
            LogStream::Stdout => &mut self.stdout,
            LogStream::Stderr => &mut self.stderr,
        };
        partial.extend_from_slice(payload);
        if let Some(end) = partial.iter().rposition(|&byte| byte == b'\n') {
            let complete: Vec<u8> = partial.drain(..=end).collect();
            split(&mut self.ready, stream, &complete);
        }
        if partial.len() > MAX_LINE_BYTES {
            split(&mut self.ready, stream, partial);
            partial.clear();
        }
    }

    // the output ended, so a line still missing its newline is complete as it is
    pub(crate) fn finish(&mut self) {
        for (stream, partial) in [
            (LogStream::Stdout, &mut self.stdout),
            (LogStream::Stderr, &mut self.stderr),
        ] {
            if !partial.is_empty() {
                split(&mut self.ready, stream, partial);
                partial.clear();
            }
        }
    }
}

fn split(ready: &mut VecDeque<LogLine>, stream: LogStream, bytes: &[u8]) {
    ready.extend(String::from_utf8_lossy(bytes).lines().map(|text| LogLine {
        stream,
        text: text.to_owned(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &Lines) -> Vec<(LogStream, &str)> {
        lines
            .ready
            .iter()
            .map(|line| (line.stream, line.text.as_str()))
            .collect()
    }

    #[test]
    fn joins_a_line_split_across_frames() {
        let mut lines = Lines::default();
        lines.push(LogStream::Stdout, b"first half, ");
        lines.push(LogStream::Stderr, b"oops\n");
        lines.push(LogStream::Stdout, b"second half\nnext\n");
        assert_eq!(
            texts(&lines),
            vec![
                (LogStream::Stderr, "oops"),
                (LogStream::Stdout, "first half, second half"),
                (LogStream::Stdout, "next"),
            ]
        );
    }

    #[test]
    fn keeps_an_unfinished_line_until_the_end() {
        let mut lines = Lines::default();
        lines.push(LogStream::Stdout, b"done\nno newline");
        assert_eq!(texts(&lines), vec![(LogStream::Stdout, "done")]);
        lines.finish();
        assert_eq!(lines.ready[1].text, "no newline");
        lines.finish();
        assert_eq!(lines.ready.len(), 2);
    }

    #[test]
    fn passes_on_a_runaway_line() {
        let mut lines = Lines::default();
        lines.push(LogStream::Stdout, &vec![b'x'; MAX_LINE_BYTES + 1]);
        assert_eq!(lines.ready.len(), 1);
        assert_eq!(lines.stdout, Vec::<u8>::new());
    }
}
