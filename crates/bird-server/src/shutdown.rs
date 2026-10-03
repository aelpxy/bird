use std::future::Future;

use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch;

#[derive(Clone)]
pub(crate) struct Shutdown {
    stopping: watch::Receiver<bool>,
}

impl Shutdown {
    pub(crate) fn on_signal() -> Self {
        let (trigger, stopping) = watch::channel(false);
        tokio::spawn(async move {
            wait_for_signal().await;
            tracing::info!("shutdown signal received");
            let _ = trigger.send(true);
        });
        Self { stopping }
    }

    pub(crate) fn wait(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut stopping = self.stopping.clone();
        async move {
            let _ = stopping.wait_for(|stop| *stop).await;
        }
    }
}

async fn wait_for_signal() {
    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(terminate) => terminate,
        Err(err) => {
            tracing::warn!(error = %err, "cannot listen for SIGTERM, only ctrl-c will stop birdd");
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}
