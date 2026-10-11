use bird_core::{ProxySettings, ServiceId};

use crate::error::expect_changed;
use crate::{Error, Result, Store, rows};

impl Store {
    pub fn proxy_settings(&self, service_id: ServiceId) -> Result<ProxySettings> {
        self.query_one(
            "SELECT proxy FROM services WHERE id = ?1",
            [service_id.to_string()],
            |row| rows::proxy(row, 0),
        )?
        .ok_or(Error::NotFound("service"))
    }

    // the default is stored as NULL, so services that never touched it read the same as before
    pub fn set_proxy_settings(
        &self,
        service_id: ServiceId,
        settings: &ProxySettings,
    ) -> Result<()> {
        let encoded = if settings.is_default() {
            None
        } else {
            Some(serde_json::to_string(settings).map_err(Error::Encode)?)
        };
        let changed = self.execute(
            "UPDATE services SET proxy = ?2 WHERE id = ?1",
            rusqlite::params![service_id.to_string(), encoded],
        )?;
        expect_changed(changed, "service")
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{Hostname, ProxySettings};

    use crate::testing::setup;

    #[test]
    fn keeps_settings_and_routes_carry_them_with_redirects() {
        let (store, service) = setup();
        assert!(store.proxy_settings(service.id).unwrap().is_default());
        let settings: ProxySettings =
            serde_json::from_str(r#"{"maintenance":true,"max_body_mb":5}"#).unwrap();
        store.set_proxy_settings(service.id, &settings).unwrap();
        assert_eq!(store.proxy_settings(service.id).unwrap(), settings);

        let apex: Hostname = "example.com".parse().unwrap();
        let www: Hostname = "www.example.com".parse().unwrap();
        store.add_domain(service.id, &apex).unwrap();
        store.add_domain(service.id, &www).unwrap();
        store.set_domain_redirect(&www, Some(&apex)).unwrap();
        let routes = store.list_routes().unwrap();
        assert_eq!(routes.len(), 2);
        assert!(routes.iter().all(|route| route.proxy == settings));
        let redirected = routes.iter().find(|route| route.hostname == www).unwrap();
        assert_eq!(redirected.redirect_to.as_ref(), Some(&apex));

        // removing the target sends the redirecting domain back to the app
        store.remove_domain(&apex).unwrap();
        let routes = store.list_routes().unwrap();
        assert_eq!(routes[0].redirect_to, None);

        store
            .set_proxy_settings(service.id, &ProxySettings::default())
            .unwrap();
        assert!(store.proxy_settings(service.id).unwrap().is_default());
    }
}
