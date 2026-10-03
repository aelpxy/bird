use std::fmt::Write;

pub(crate) fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_characters() {
        assert_eq!(encode("bird-web_1.x~"), "bird-web_1.x~");
        assert_eq!(
            encode("docker.io/library/nginx:alpine"),
            "docker.io%2Flibrary%2Fnginx%3Aalpine"
        );
        assert_eq!(
            encode(r#"{"label":["a=b"]}"#),
            "%7B%22label%22%3A%5B%22a%3Db%22%5D%7D"
        );
    }
}
