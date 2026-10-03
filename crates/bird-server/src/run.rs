use std::future::IntoFuture;
use std::sync::Arc;

use bird_core::{EnvironmentId, Name};
use bird_podman::{Podman, default_socket};
use bird_proxy::{Proxy, ProxyConfig, Routes};
use bird_store::Store;
use tokio::net::TcpListener;

use crate::db::Db;
use crate::deploy::DeployGuard;
use crate::shutdown::Shutdown;
use crate::state::AppState;
use crate::{Config, Result, api, reconcile, routing};

const DEFAULT_PROJECT: &str = "default";
const DEFAULT_ENVIRONMENT: &str = "production";

pub async fn run(config: Config) -> Result<()> {
    let data_dir = config.data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let db = Db::open(&data_dir.join("bird.db"))?;

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
    reconcile::run(&state).await;
    routing::refresh(&state).await?;

    let api_listener = TcpListener::bind(config.api_addr).await?;
    let proxy_listener = TcpListener::bind(config.proxy_addr).await?;
    tracing::info!(
        api = %config.api_addr,
        proxy = %config.proxy_addr,
        data = %data_dir.display(),
        podman = %state.podman.socket().display(),
        "birdd ready"
    );

    let shutdown = Shutdown::on_signal();
    let proxy = Proxy::new(state.routes.clone(), ProxyConfig::default());
    let api = axum::serve(api_listener, api::router(state))
        .with_graceful_shutdown(shutdown.wait())
        .into_future();
    let ((), api_result) = tokio::join!(proxy.serve(proxy_listener, shutdown.wait()), api);
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
