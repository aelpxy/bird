use std::path::Path;

use bird_store::Store;
use tokio::sync::{mpsc, oneshot};

use crate::{Error, Result};

const QUEUE_DEPTH: usize = 256;

type Job = Box<dyn FnOnce(&mut Store) + Send>;

#[derive(Clone)]
pub(crate) struct Db {
    jobs: mpsc::Sender<Job>,
}

impl Db {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        Self::spawn(Store::open(path)?)
    }

    fn spawn(mut store: Store) -> Result<Self> {
        let (jobs, mut queue) = mpsc::channel::<Job>(QUEUE_DEPTH);
        std::thread::Builder::new()
            .name("bird-db".to_owned())
            .spawn(move || {
                while let Some(job) = queue.blocking_recv() {
                    job(&mut store);
                }
            })?;
        Ok(Self { jobs })
    }

    pub(crate) async fn call<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store) -> bird_store::Result<T> + Send + 'static,
    {
        let (reply, result) = oneshot::channel();
        let job: Job = Box::new(move |store| {
            let _ = reply.send(work(store));
        });
        self.jobs.send(job).await.map_err(|_| Error::DbClosed)?;
        Ok(result.await.map_err(|_| Error::DbClosed)??)
    }
}

#[cfg(test)]
impl Db {
    pub(crate) fn in_memory() -> Self {
        Self::spawn(Store::open_in_memory().unwrap()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use bird_core::Name;

    use super::*;

    #[tokio::test]
    async fn runs_jobs_on_worker_thread() {
        let db = Db::in_memory();
        let name: Name = "default".parse().unwrap();
        let created = db
            .call({
                let name = name.clone();
                move |store| store.create_project(&name)
            })
            .await
            .unwrap();
        let found = db
            .call(move |store| store.project_by_name(&name))
            .await
            .unwrap();
        assert_eq!(found, Some(created));
    }

    #[tokio::test]
    async fn surfaces_store_errors() {
        let db = Db::in_memory();
        let name: Name = "default".parse().unwrap();
        let create = move |store: &mut Store| store.create_project(&name);
        db.call(create.clone()).await.unwrap();
        assert!(matches!(
            db.call(create).await.unwrap_err(),
            Error::Store(bird_store::Error::AlreadyExists("project"))
        ));
    }
}
