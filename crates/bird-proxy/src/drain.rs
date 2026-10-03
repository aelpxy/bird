use tokio::sync::{mpsc, watch};

// hyper-util's graceful tracker cannot watch upgradeable connections, so connections and
// websocket tunnels hold a Watch and shutdown waits until every Watch has been dropped
pub(crate) struct Drain {
    signal: watch::Sender<bool>,
    alive: mpsc::Sender<()>,
    finished: mpsc::Receiver<()>,
}

#[derive(Clone)]
pub(crate) struct Watch {
    signal: watch::Receiver<bool>,
    _alive: mpsc::Sender<()>,
}

impl Drain {
    pub(crate) fn new() -> Self {
        let (signal, _) = watch::channel(false);
        let (alive, finished) = mpsc::channel(1);
        Self {
            signal,
            alive,
            finished,
        }
    }

    pub(crate) fn watch(&self) -> Watch {
        Watch {
            signal: self.signal.subscribe(),
            _alive: self.alive.clone(),
        }
    }

    pub(crate) async fn shutdown(self) {
        let Self {
            signal,
            alive,
            mut finished,
        } = self;
        signal.send_replace(true);
        drop(alive);
        let _ = finished.recv().await;
    }
}

impl Watch {
    pub(crate) async fn signaled(&mut self) {
        let _ = self.signal.wait_for(|stop| *stop).await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn waits_for_every_watch_to_finish() {
        let drain = Drain::new();
        let mut watch = drain.watch();
        let worker = tokio::spawn(async move {
            watch.signaled().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        });
        tokio::time::timeout(Duration::from_secs(1), drain.shutdown())
            .await
            .unwrap();
        assert!(worker.is_finished());
    }

    #[tokio::test]
    async fn finishes_immediately_without_watchers() {
        tokio::time::timeout(Duration::from_millis(100), Drain::new().shutdown())
            .await
            .unwrap();
    }
}
