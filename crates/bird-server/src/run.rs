use std::future::IntoFuture;
use std::sync::Arc;

use bird_core::{EnvironmentId, Name};
use bird_podman::{Podman, default_socket};
use bird_proxy::{Proxy, ProxyConfig, Routes};
use bird_store::Store;

use crate::db::Db;
use crate::deploy::DeployGuard;
use crate::shutdown::Shutdown;
use crate::state::AppState;
use crate::supervisor::{self, Supervisor};
use crate::token::ApiToken;
use crate::{Config, Result, api};

const DEFAULT_PROJECT: &str = "default";
const DEFAULT_ENVIRONMENT: &str = "production";

pub async fn run(config: Config) -> Result<()> {
    let data_dir = config.data_dir();
    std::fs::create_dir_all(&data_dir)?;
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

    let state = AppState {
        db,
        podman,
        routes: Routes::new(),
        environment_id,
        network: Arc::from(config.network.as_str()),
        deploys: DeployGuard::default(),
    };
    supervisor::recover_interrupted(&state).await?;
    let mut supervisor = Supervisor::new(state.clone());
    supervisor.sweep().await;

    let api_listener = bird_proxy::bind(config.api_addr)?;
    let proxy_listener = bird_proxy::bind(config.proxy_addr)?;
    tracing::info!(
        api = %config.api_addr,
        proxy = %config.proxy_addr,
        data = %data_dir.display(),
        token = %token_path.display(),
        podman = %state.podman.socket().display(),
        "birdd ready"
    );

    if !config.api_addr.ip().is_loopback() {
        tracing::warn!(api = %config.api_addr, "api is reachable over the network without tls, the token is sent in plain text");
    }

    let shutdown = Shutdown::on_signal();
    let proxy = Proxy::new(state.routes.clone(), ProxyConfig::default());
    let api = axum::serve(api_listener, api::router(state, token))
        .with_graceful_shutdown(shutdown.wait())
        .into_future();
    let ((), api_result, ()) = tokio::join!(
        proxy.serve(proxy_listener, shutdown.wait()),
        api,
        supervisor.run(shutdown.wait())
    );
    api_result?;
    tracing::info!("birdd stopped");
    Ok(())
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
