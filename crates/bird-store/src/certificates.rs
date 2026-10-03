use bird_core::{Certificate, Hostname};
use rusqlite::params;

use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn put_certificate(&self, certificate: &Certificate) -> Result<()> {
        self.execute(
            "INSERT INTO certificates (hostname, chain_pem, key_pem, not_after, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (hostname) DO UPDATE SET chain_pem = excluded.chain_pem,
                 key_pem = excluded.key_pem, not_after = excluded.not_after,
                 updated_at = excluded.updated_at",
            params![
                certificate.hostname.as_str(),
                certificate.chain_pem,
                certificate.key_pem,
                certificate.not_after,
                now()
            ],
        )?;
        Ok(())
    }

    pub fn list_certificates(&self) -> Result<Vec<Certificate>> {
        self.query_all(
            "SELECT c.hostname, c.chain_pem, c.key_pem, c.not_after
             FROM certificates c JOIN domains d ON d.hostname = c.hostname
             ORDER BY c.hostname",
            [],
            rows::certificate,
        )
    }

    pub fn list_hostnames(&self) -> Result<Vec<Hostname>> {
        self.query_all(
            "SELECT hostname FROM domains ORDER BY hostname",
            [],
            rows::hostname,
        )
    }

    pub fn acme_credentials(&self, directory_url: &str) -> Result<Option<String>> {
        self.query_one(
            "SELECT credentials FROM acme_accounts WHERE directory_url = ?1",
            [directory_url],
            |row| row.get(0),
        )
    }

    pub fn save_acme_credentials(&self, directory_url: &str, credentials: &str) -> Result<()> {
        self.execute(
            "INSERT INTO acme_accounts (directory_url, credentials, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (directory_url) DO UPDATE SET credentials = excluded.credentials",
            params![directory_url, credentials, now()],
        )?;
        Ok(())
    }

    pub fn forget_acme_credentials(&self, directory_url: &str) -> Result<()> {
        self.execute(
            "DELETE FROM acme_accounts WHERE directory_url = ?1",
            [directory_url],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{Certificate, Hostname};

    use crate::testing::setup;

    fn certificate(host: &str, not_after: i64) -> Certificate {
        Certificate {
            hostname: host.parse().unwrap(),
            chain_pem: "chain".to_owned(),
            key_pem: "key".to_owned(),
            not_after,
        }
    }

    #[test]
    fn upserts_and_lists_only_current_domains() {
        let (store, service) = setup();
        let host: Hostname = "web.example.com".parse().unwrap();
        store.add_domain(service.id, &host).unwrap();

        store
            .put_certificate(&certificate("web.example.com", 1))
            .unwrap();
        store
            .put_certificate(&certificate("web.example.com", 2))
            .unwrap();
        store
            .put_certificate(&certificate("gone.example.com", 3))
            .unwrap();

        let listed = store.list_certificates().unwrap();
        assert_eq!(listed, vec![certificate("web.example.com", 2)]);
        assert_eq!(store.list_hostnames().unwrap(), vec![host]);
    }

    #[test]
    fn stores_acme_credentials_per_directory() {
        let (store, _) = setup();
        assert_eq!(store.acme_credentials("https://ca/dir").unwrap(), None);
        store.save_acme_credentials("https://ca/dir", "{}").unwrap();
        store
            .save_acme_credentials("https://ca/dir", "{\"a\":1}")
            .unwrap();
        assert_eq!(
            store.acme_credentials("https://ca/dir").unwrap().as_deref(),
            Some("{\"a\":1}")
        );
        store.forget_acme_credentials("https://ca/dir").unwrap();
        assert_eq!(store.acme_credentials("https://ca/dir").unwrap(), None);
    }
}
