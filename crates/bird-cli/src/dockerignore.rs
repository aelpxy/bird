// a client-side copy of .dockerignore rules that only trims the upload; podman applies the real
// rules again, so anything not understood here is simply sent
pub(crate) struct DockerIgnore {
    rules: Vec<Rule>,
}

struct Rule {
    exception: bool,
    parts: Vec<String>,
}

impl DockerIgnore {
    pub(crate) fn parse(text: &str) -> Self {
        let rules: Vec<Rule> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let (exception, pattern) = match line.strip_prefix('!') {
                    Some(rest) => (true, rest.trim()),
                    None => (false, line),
                };
                let parts: Vec<String> = pattern
                    .split('/')
                    .filter(|part| !part.is_empty() && *part != ".")
                    .map(str::to_owned)
                    .collect();
                (!parts.is_empty()).then_some(Rule { exception, parts })
            })
            .collect();
        // character classes and escapes are not handled here, so send everything instead of guessing
        let unsupported = rules
            .iter()
            .flat_map(|rule| &rule.parts)
            .any(|part| part.contains(['[', '\\']));
        Self {
            rules: if unsupported { Vec::new() } else { rules },
        }
    }

    // a directory can be skipped whole only when no exception could bring something back
    pub(crate) fn skips_dir(&self, path: &str) -> bool {
        !self.rules.iter().any(|rule| rule.exception) && self.excludes(path)
    }

    // like docker, a rule matches the path itself or any directory above it, and the last match wins
    pub(crate) fn excludes(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        let mut excluded = false;
        for rule in &self.rules {
            let matched = (1..=parts.len())
                .filter_map(|depth| parts.get(..depth))
                .any(|prefix| glob_parts(&rule.parts, prefix));
            if matched {
                excluded = !rule.exception;
            }
        }
        excluded
    }
}

fn glob_parts(pattern: &[String], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((first, rest)) if first == "**" => (0..=path.len())
            .filter_map(|skip| path.get(skip..))
            .any(|tail| glob_parts(rest, tail)),
        Some((first, rest)) => path.split_first().is_some_and(|(head, tail)| {
            glob_segment(first.as_bytes(), head.as_bytes()) && glob_parts(rest, tail)
        }),
    }
}

fn glob_segment(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len())
            .filter_map(|skip| text.get(skip..))
            .any(|tail| glob_segment(rest, tail)),
        Some((b'?', rest)) => text
            .split_first()
            .is_some_and(|(_, tail)| glob_segment(rest, tail)),
        Some((expected, rest)) => text
            .split_first()
            .is_some_and(|(actual, tail)| actual == expected && glob_segment(rest, tail)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_like_docker() {
        let ignore = DockerIgnore::parse(
            "# deps\nnode_modules\n/.git/\n*.log\n**/tmp\ndocs/*.md\n!docs/keep.md\n",
        );
        for excluded in [
            "node_modules",
            "node_modules/a/b.js",
            ".git/HEAD",
            "app.log",
            "a/b/tmp",
            "a/b/tmp/x",
            "tmp",
            "docs/readme.md",
        ] {
            assert!(ignore.excludes(excluded), "{excluded}");
        }
        for included in [
            "src/main.rs",
            "nested/app.log",
            "docs/keep.md",
            "docs/sub/a.md",
        ] {
            assert!(!ignore.excludes(included), "{included}");
        }
    }

    #[test]
    fn exceptions_stop_skipping_whole_directories() {
        let plain = DockerIgnore::parse("vendor\n");
        assert!(plain.skips_dir("vendor"));
        let with_exception = DockerIgnore::parse("vendor\n!vendor/keep\n");
        assert!(!with_exception.skips_dir("vendor"));
        assert!(with_exception.excludes("vendor/other"));
        assert!(!with_exception.excludes("vendor/keep"));
    }

    #[test]
    fn unsupported_patterns_send_everything() {
        let ignore = DockerIgnore::parse("node_modules\n[ab].txt\n");
        assert!(!ignore.excludes("node_modules"));
    }
}
