use std::fmt::Write as _;

use ring::hmac;

// rfc 6238 with what every authenticator app expects: sha-1, 30 second steps, 6 digits
const STEP_SECS: i64 = 30;
const DIGITS: u32 = 6;
const SECRET_BYTES: usize = 20;
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
// a code from the step before or after also counts, for clocks that drift a little
const DRIFT_STEPS: i64 = 1;

pub(crate) fn new_secret() -> std::io::Result<String> {
    Ok(encode(&crate::secrets::random_bytes(SECRET_BYTES)?))
}

// what authenticator apps read from a qr code
pub(crate) fn uri(secret: &str, account: &str) -> String {
    format!(
        "otpauth://totp/bird:{account}?secret={secret}&issuer=bird&algorithm=SHA1&digits={DIGITS}&period={STEP_SECS}"
    )
}

// the step `code` is valid for around `now`, if any; the caller makes sure each step is used once
pub(crate) fn verify(secret: &str, code: &str, now_secs: i64) -> Option<i64> {
    let key = decode(secret)?;
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let current = now_secs.div_euclid(STEP_SECS);
    (current - DRIFT_STEPS..=current + DRIFT_STEPS)
        .find(|step| constant_time_eq(code_at(&key, *step).as_bytes(), code.as_bytes()))
}

fn code_at(key: &[u8], step: i64) -> String {
    let counter = u64::try_from(step).unwrap_or(0).to_be_bytes();
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key),
        &counter,
    );
    let digest = tag.as_ref();
    let offset = usize::from(digest.last().copied().unwrap_or(0) & 0x0f);
    let word = digest
        .get(offset..offset + 4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map_or(0, u32::from_be_bytes)
        & 0x7fff_ffff;
    let mut code = String::with_capacity(6);
    let _ = write!(code, "{:06}", word % 10_u32.pow(DIGITS));
    code
}

pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buffer, mut bits) = (0_u32, 0_u32);
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            push_symbol(&mut out, buffer >> bits);
        }
    }
    if bits > 0 {
        push_symbol(&mut out, buffer << (5 - bits));
    }
    out
}

fn push_symbol(out: &mut String, value: u32) {
    let index = usize::try_from(value & 0x1f).unwrap_or(0);
    if let Some(symbol) = BASE32.get(index) {
        out.push(char::from(*symbol));
    }
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0_u32, 0_u32);
    for symbol in text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
    {
        let value = BASE32
            .iter()
            .position(|known| *known == symbol.to_ascii_uppercase())?;
        buffer = (buffer << 5) | u32::try_from(value).ok()?;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xff).ok()?);
        }
    }
    Some(out)
}

// no early exit, so timing does not tell how many digits were right
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // the sha-1 vectors from rfc 6238, cut to 6 digits
    #[test]
    fn matches_the_rfc_vectors() {
        let key = b"12345678901234567890";
        for (time, code) in [
            (59, "287082"),
            (1_111_111_109, "081804"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
        ] {
            assert_eq!(code_at(key, time / STEP_SECS), code, "{time}");
        }
    }

    #[test]
    fn base32_roundtrips() {
        assert_eq!(
            encode(b"12345678901234567890"),
            "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        );
        assert_eq!(
            decode("gezdgnbvgy3tqojqgezdgnbvgy3tqojq").unwrap(),
            b"12345678901234567890"
        );
        assert_eq!(decode("not base32!"), None);
        let secret = new_secret().unwrap();
        assert_eq!(decode(&secret).unwrap().len(), SECRET_BYTES);
    }

    #[test]
    fn accepts_a_step_of_drift_only() {
        let secret = encode(b"12345678901234567890");
        assert_eq!(verify(&secret, "287 082", 59), Some(1));
        assert_eq!(verify(&secret, "287082", 59 + 30), Some(1));
        assert_eq!(verify(&secret, "287082", 59 + 90), None);
        assert_eq!(verify(&secret, "000000", 59), None);
        assert!(uri(&secret, "ada").starts_with("otpauth://totp/bird:ada?secret=GEZDGNBV"));
    }
}
