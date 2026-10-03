use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use bird_core::Name;

use crate::{Error, Result};

#[derive(Clone, Default)]
pub(crate) struct DeployGuard {
    busy: Arc<Mutex<HashSet<Name>>>,
}

pub(crate) struct Ticket {
    guard: DeployGuard,
    name: Name,
}

impl DeployGuard {
    pub(crate) fn begin(&self, name: &Name) -> Result<Ticket> {
        let mut busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        if !busy.insert(name.clone()) {
            return Err(Error::Busy(name.clone()));
        }
        Ok(Ticket {
            guard: self.clone(),
            name: name.clone(),
        })
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.guard
            .busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_one_operation_per_service() {
        let guard = DeployGuard::default();
        let web: Name = "web".parse().unwrap();
        let api: Name = "api".parse().unwrap();

        let ticket = guard.begin(&web).unwrap();
        assert!(matches!(guard.begin(&web), Err(Error::Busy(_))));
        let _other = guard.begin(&api).unwrap();

        drop(ticket);
        assert!(guard.begin(&web).is_ok());
    }
}
