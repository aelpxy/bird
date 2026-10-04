use crate::ui::style::{self, Paint};

// cells may carry color codes, so widths count only the characters a terminal shows
pub(crate) fn render(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = header.iter().map(|h| h.len()).collect();
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(visible_len(cell));
        }
    }
    let header: Vec<String> = header.iter().map(|h| style::out(Paint::Dim, h)).collect();
    let mut out = String::new();
    for row in std::iter::once(&header).chain(rows) {
        let mut line = String::new();
        for (cell, width) in row.iter().zip(&widths) {
            line.push_str(cell);
            line.push_str(&" ".repeat(width.saturating_sub(visible_len(cell)) + 2));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn visible_len(text: &str) -> usize {
    let mut len = 0;
    let mut escaped = false;
    for c in text.chars() {
        match (escaped, c) {
            (false, '\x1b') => escaped = true,
            (true, 'm') => escaped = false,
            (true, _) => {}
            (false, _) => len += 1,
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns() {
        let rows = vec![vec![
            "web".to_owned(),
            "nginx:alpine".to_owned(),
            "\x1b[32mactive\x1b[0m".to_owned(),
        ]];
        let output = render(&["NAME", "IMAGE", "STATUS"], &rows);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "NAME  IMAGE         STATUS");
        assert_eq!(lines[1], "web   nginx:alpine  \x1b[32mactive\x1b[0m");
    }

    #[test]
    fn counts_visible_characters() {
        assert_eq!(visible_len("\x1b[1mab\x1b[0mc"), 3);
        assert_eq!(visible_len("⠋ ok"), 4);
    }
}
