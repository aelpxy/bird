pub(crate) fn render(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = header.iter().map(|h| h.len()).collect();
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let header: Vec<String> = header.iter().map(|h| (*h).to_owned()).collect();
    let mut out = String::new();
    for row in std::iter::once(&header).chain(rows) {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns() {
        let rows = vec![vec![
            "web".to_owned(),
            "nginx:alpine".to_owned(),
            "active".to_owned(),
        ]];
        let output = render(&["NAME", "IMAGE", "STATUS"], &rows);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "NAME  IMAGE         STATUS");
        assert_eq!(lines[1], "web   nginx:alpine  active");
    }
}
