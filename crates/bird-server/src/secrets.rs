use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;

use bird_core::EnvKey;
use bird_core::reference::expand_secrets;

use crate::Result;

const ALPHABET: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
// largest multiple of 62 that fits in a byte, so every symbol stays equally likely
const UNBIASED_LIMIT: u8 = 248;

pub(crate) fn expand_all(variables: BTreeMap<EnvKey, String>) -> Result<BTreeMap<EnvKey, String>> {
    variables
        .into_iter()
        .map(|(key, value)| Ok((key, expand_secrets(&value, generate)??)))
        .collect()
}

pub(crate) fn generate(length: usize) -> std::io::Result<String> {
    let mut random = File::open("/dev/urandom")?;
    let mut secret = String::with_capacity(length);
    let mut buffer = [0_u8; 64];
    while secret.len() < length {
        random.read_exact(&mut buffer)?;
        for byte in buffer.iter().filter(|b| **b < UNBIASED_LIMIT) {
            if secret.len() == length {
                break;
            }
            if let Some(symbol) = ALPHABET.get(usize::from(byte % 62)) {
                secret.push(char::from(*symbol));
            }
        }
    }
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_distinct_alphanumeric_secrets() {
        let a = generate(48).unwrap();
        let b = generate(48).unwrap();
        assert_eq!(a.len(), 48);
        assert!(a.bytes().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(a, b);
    }

    #[test]
    fn expands_only_secret_placeholders() {
        let input = BTreeMap::from([
            ("PASSWORD".parse().unwrap(), "${{secret(20)}}".to_owned()),
            ("URL".parse().unwrap(), "${{pg.URL}}".to_owned()),
        ]);
        let expanded = expand_all(input).unwrap();
        let password = &expanded[&"PASSWORD".parse().unwrap()];
        assert_eq!(password.len(), 20);
        assert_eq!(expanded[&"URL".parse().unwrap()], "${{pg.URL}}");
    }

    #[test]
    fn rejects_malformed_references() {
        let input = BTreeMap::from([("A".parse().unwrap(), "${{broken".to_owned())]);
        assert!(expand_all(input).is_err());
    }
}
