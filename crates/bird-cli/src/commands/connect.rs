use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bird_api::{ServiceSummary, TCP_UPGRADE};
use bird_core::Name;
use tokio::net::{TcpListener, TcpStream};

use super::build::encode;
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};

const TIMEOUT: Duration = Duration::from_secs(30);

// each local connection gets its own tunnel, so clients that pool connections work as usual
pub(crate) async fn run(
    client: ApiClient,
    name: &Name,
    local_port: Option<u16>,
    machine: Option<&str>,
) -> Result<()> {
    let service: ServiceSummary = client
        .get(&client.scoped(&format!("services/{name}")), TIMEOUT)
        .await?;
    let remote = service.port.get();
    let port = local_port.unwrap_or(remote);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .with_context(|| {
            format!("cannot listen on localhost:{port}, pick another with --local-port")
        })?;
    let mut path = client.scoped(&format!("services/{name}/connect"));
    if let Some(machine) = machine {
        path.push_str("?machine=");
        path.push_str(&encode(machine));
    }
    eprintln!(
        "{} forwarding {} to {name}:{remote}, press Ctrl-C to stop",
        style::err(Paint::Cyan, "›"),
        style::err(Paint::Bold, format!("localhost:{port}"))
    );
    let (client, path) = (Arc::new(client), Arc::new(path));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (local, _) = accepted.context("could not accept a local connection")?;
                tokio::spawn(forward(Arc::clone(&client), Arc::clone(&path), local));
            }
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

async fn forward(client: Arc<ApiClient>, path: Arc<String>, mut local: TcpStream) {
    let mut remote = match client.upgrade(&path, TCP_UPGRADE, TIMEOUT).await {
        Ok(remote) => remote,
        Err(err) => {
            eprintln!("{} {err:#}", style::err(Paint::Red, "error:"));
            return;
        }
    };
    if let Err(err) = tokio::io::copy_bidirectional(&mut local, &mut remote).await {
        eprintln!(
            "{} {}",
            style::err(Paint::Dim, "connection closed:"),
            style::err(Paint::Dim, err)
        );
    }
}
