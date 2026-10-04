use std::future::IntoFuture;
use std::path::Path;
use std::sync::Arc;

use bird_core::{EnvironmentId, Name};
use bird_podman::{Podman, default_socket};
use bird_proxy::{CertStore, Challenges, Proxy, ProxyConfig, Routes, Tls};
use bird_store::Store;
use tokio::sync::{Notify, Semaphore};

use crate::backups::{BackupStorage, LocalDir};
use crate::db::Db;
use crate::deploy::DeployGuard;
use crate::shutdown::Shutdown;
use crate::state::AppState;
use crate::supervisor::{self, Supervisor};
use crate::tls::{self, AcmeSettings, CertManager};
use crate::token::ApiToken;
use crate::{Config, Error, Result, api, data_dir, listen};

const DEFAULT_PROJECT: &str = "default";
const DEFAULT_ENVIRONMENT: &str = "production";

// builds use a lot of cpu and memory, one at a time keeps a small server responsive
const MAX_CONCURRENT_BUILDS: usize = 1;

pub async fn run(config: Config) -> Result<()> {
    let data_dir = config.data_dir();
    data_dir::prepare(&data_dir)?;
    let db = Db::open(&data_dir.join("bird.db"))?;
    let token_path = data_dir.join("api-token");
    let token = ApiToken::load_or_create(&token_path)?;

    let project: Name = DEFAULT_PROJECT.parse()?;
    let environment: Name = DEFAULT_ENVIRONMENT.parse()?;
    let environment_id = db
        .call(move |store| ensure_environment(store, &project, &environment))
        .await?;

    let podman = Podman::new(config.podman_socket.clone().unwrap_or_else(default_socket));
    podman.ping().await?;
    podman.ensure_network(&config.network).await?;

    let shutdown = Shutdown::on_signal();
    let state = AppState {
        db,
        podman,
        routes: Routes::new(),
        environment_id,
        network: Arc::from(config.network.as_str()),
        deploys: DeployGuard::default(),
        domains_changed: Arc::new(Notify::new()),
        reconcile_now: Arc::new(Notify::new()),
        shutdown: shutdown.clone(),
        builds: Arc::new(Semaphore::new(MAX_CONCURRENT_BUILDS)),
        backups: Arc::new(backup_storage(&config, &data_dir)),
    };
    supervisor::recover_interrupted(&state).await?;
    let mut supervisor = Supervisor::new(state.clone());
    supervisor.sweep().await;

    let edge = match &config.acme_directory {
        Some(directory) => {
            let tls = Tls {
                certificates: CertStore::new(),
                challenges: Challenges::new(),
                https_port: config.https_addr.port(),
            };
            tls::load_certificates(&state, &tls.certificates).await?;
            let settings = AcmeSettings {
                directory: directory.clone(),
                email: config.acme_email.clone(),
                ca_cert: config.acme_ca_cert.clone(),
            };
            Some((tls, settings))
        }
        None => None,
    };

    let api_listener = listen::bind(config.api_addr)?;
    let proxy_listener = listen::bind(config.proxy_addr)?;
    let https_listener = match edge {
        Some(_) => Some(listen::bind(config.https_addr)?),
        None => None,
    };
    tracing::info!(
        api = %config.api_addr,
        proxy = %config.proxy_addr,
        https = ?edge.as_ref().map(|_| config.https_addr),
        data = %data_dir.display(),
        token = %token_path.display(),
        podman = %state.podman.socket().display(),
        "birdd ready"
    );

    if !config.api_addr.ip().is_loopback() {
        tracing::warn!(api = %config.api_addr, "api is reachable over the network without tls, the token is sent in plain text");
    }

    let proxy = Proxy::new(
        state.routes.clone(),
        ProxyConfig::default(),
        edge.as_ref().map(|(tls, _)| tls.clone()),
    );
    let certificates = edge.map(|(tls, settings)| {
        CertManager::new(state.clone(), settings, tls.certificates, tls.challenges)
    });
    let https = async {
        match https_listener {
            Some(listener) => proxy.serve_tls(listener, shutdown.wait()).await,
            None => Ok(()),
        }
    };
    let renewals = async {
        if let Some(manager) = certificates {
            manager.run(shutdown.wait()).await;
        }
    };
    let api = axum::serve(api_listener, api::router(state, token))
        .with_graceful_shutdown(shutdown.wait())
        .into_future();
    let ((), https_result, api_result, (), ()) = tokio::join!(
        proxy.serve(proxy_listener, shutdown.wait()),
        https,
        api,
        supervisor.run(shutdown.wait()),
        renewals
    );
    https_result.map_err(|err| Error::Certificate(err.to_string()))?;
    api_result?;
    tracing::info!("birdd stopped");
    Ok(())
}

fn backup_storage(config: &Config, data_dir: &Path) -> BackupStorage {
    let dir = config
        .backup_dir
        .clone()
        .unwrap_or_else(|| data_dir.join("backups"));
    BackupStorage::Local(LocalDir::new(dir))
}

fn ensure_environment(
    store: &mut Store,
    project: &Name,
    environment: &Name,
) -> bird_store::Result<EnvironmentId> {
    store.transaction(|store| {
        let project = match store.project_by_name(project)? {
            Some(existing) => existing,
            None => store.create_project(project)?,
        };
        let environment = match store.environment_by_name(project.id, environment)? {
            Some(existing) => existing,
            None => store.create_environment(project.id, environment)?,
        };
        Ok(environment.id)
    })
}
