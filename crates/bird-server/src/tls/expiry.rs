use x509_parser::pem::parse_x509_pem;

use crate::{Error, Result};

pub(super) fn not_after(chain_pem: &str) -> Result<i64> {
    let (_, pem) = parse_x509_pem(chain_pem.as_bytes())
        .map_err(|err| Error::Certificate(format!("unreadable pem: {err}")))?;
    let certificate = pem
        .parse_x509()
        .map_err(|err| Error::Certificate(format!("unreadable x509: {err}")))?;
    Ok(certificate.validity().not_after.timestamp())
}

#[cfg(test)]
mod tests {
    use rcgen::{CertificateParams, KeyPair, date_time_ymd};

    use super::*;

    #[test]
    fn reads_leaf_expiry() {
        let mut params = CertificateParams::new(vec!["web.example.com".to_owned()]).unwrap();
        params.not_after = date_time_ymd(2030, 1, 1);
        let certificate = params.self_signed(&KeyPair::generate().unwrap()).unwrap();
        assert_eq!(not_after(&certificate.pem()).unwrap(), 1_893_456_000);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            not_after("not a certificate"),
            Err(Error::Certificate(_))
        ));
    }
}
