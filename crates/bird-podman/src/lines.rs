use std::collections::VecDeque;

use crate::logs::{LogLine, LogStream};
use crate::timestamp::split_timestamp;

// a line longer than this is passed on in pieces rather than held in memory
const MAX_LINE_BYTES: usize = 1024 * 1024;

// podman's log driver stores a long line as several entries and returns each as a frame without a
// trailing newline, so frames are joined back until one ends the line
#[derive(Default)]
pub(crate) struct Lines {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    // when the line being joined started, from its first piece
    stdout_at: Option<i64>,
    stderr_at: Option<i64>,
    // podman was asked for timestamps, so every entry starts with one
    timestamped: bool,
    pub(crate) ready: VecDeque<LogLine>,
}

impl Lines {
    pub(crate) fn timestamped() -> Self {
        Self {
            timestamped: true,
            ..Self::default()
        }
    }

    pub(crate) fn push(&mut self, stream: LogStream, payload: &[u8]) {
        if !self.timestamped {
            self.append(stream, payload, None);
            return;
        }
        // each piece of a split line carries its own time; the line keeps its first piece's
        for entry in payload.split_inclusive(|&byte| byte == b'\n') {
            let (at, text) = split_timestamp(entry);
            self.append(stream, text, at);
        }
    }

    fn append(&mut self, stream: LogStream, bytes: &[u8], at: Option<i64>) {
        let (partial, started) = match stream {
            LogStream::Stdout => (&mut self.stdout, &mut self.stdout_at),
            LogStream::Stderr => (&mut self.stderr, &mut self.stderr_at),
        };
        if partial.is_empty() {
            *started = at;
        }
        partial.extend_from_slice(bytes);
        if let Some(end) = partial.iter().rposition(|&byte| byte == b'\n') {
            let complete: Vec<u8> = partial.drain(..=end).collect();
            split(&mut self.ready, stream, &complete, *started);
            *started = if partial.is_empty() { None } else { at };
        }
        if partial.len() > MAX_LINE_BYTES {
            split(&mut self.ready, stream, partial, started.take());
            partial.clear();
        }
    }

    // the output ended, so a line still missing its newline is complete as it is
    pub(crate) fn finish(&mut self) {
        for (stream, partial, started) in [
            (LogStream::Stdout, &mut self.stdout, &mut self.stdout_at),
            (LogStream::Stderr, &mut self.stderr, &mut self.stderr_at),
        ] {
            if !partial.is_empty() {
                split(&mut self.ready, stream, partial, started.take());
                partial.clear();
            }
        }
    }
}

fn split(ready: &mut VecDeque<LogLine>, stream: LogStream, bytes: &[u8], at: Option<i64>) {
    ready.extend(String::from_utf8_lossy(bytes).lines().map(|text| LogLine {
        stream,
        text: text.to_owned(),
        at,
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
    fn keeps_the_first_pieces_time_for_a_joined_line() {
        let mut lines = Lines::timestamped();
        lines.push(LogStream::Stdout, b"1970-01-01T00:00:01Z first ");
        lines.push(LogStream::Stdout, b"1970-01-01T00:00:02Z half\n");
        lines.push(LogStream::Stderr, b"1970-01-01T00:00:03Z oops\n");
        let got: Vec<(&str, Option<i64>)> = lines
            .ready
            .iter()
            .map(|line| (line.text.as_str(), line.at))
            .collect();
        assert_eq!(got, vec![("first half", Some(1000)), ("oops", Some(3000))]);
    }

    #[test]
    fn keeps_entries_without_a_time_whole() {
        let mut lines = Lines::timestamped();
        lines.push(LogStream::Stdout, b"plain text\n");
        assert_eq!(lines.ready[0].text, "plain text");
        assert_eq!(lines.ready[0].at, None);
    }

    #[test]
    fn passes_on_a_runaway_line() {
        let mut lines = Lines::default();
        lines.push(LogStream::Stdout, &vec![b'x'; MAX_LINE_BYTES + 1]);
        assert_eq!(lines.ready.len(), 1);
        assert_eq!(lines.stdout, Vec::<u8>::new());
    }
}
