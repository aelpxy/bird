use std::fmt;

use serde::Serialize;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

#[derive(Clone)]
pub struct RegistryAuth {
    pub username: String,
    pub password: String,
    pub tls_verify: bool,
}

impl fmt::Debug for RegistryAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryAuth")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("tls_verify", &self.tls_verify)
            .finish()
    }
}

#[derive(Serialize)]
struct Credentials<'a> {
    username: &'a str,
    password: &'a str,
}

impl RegistryAuth {
    // podman takes registry credentials as base64url-encoded json in X-Registry-Auth
    pub(crate) fn header(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_vec(&Credentials {
            username: &self.username,
            password: &self.password,
        })?;
        Ok(base64url(&json))
    }
}

fn base64url(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let bytes = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                let index = usize::try_from((n >> (18 - 6 * i)) & 0x3f).unwrap_or(0);
                out.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_rfc4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64url(input.as_bytes()), expected);
        }
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8=");
    }

    #[test]
    fn hides_the_password() {
        let auth = RegistryAuth {
            username: "u".to_owned(),
            password: "hunter2".to_owned(),
            tls_verify: true,
        };
        assert!(!format!("{auth:?}").contains("hunter2"));
        assert_eq!(
            auth.header().unwrap(),
            base64url(br#"{"username":"u","password":"hunter2"}"#)
        );
    }
}
