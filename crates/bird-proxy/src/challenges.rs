use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Clone, Default)]
pub struct Challenges {
    tokens: Arc<Mutex<HashMap<String, String>>>,
}

impl Challenges {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, token: String, key_authorization: String) {
        self.lock().insert(token, key_authorization);
    }

    pub fn remove(&self, token: &str) {
        self.lock().remove(token);
    }

    pub(crate) fn key_authorization(&self, token: &str) -> Option<String> {
        self.lock().get(token).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.tokens.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_removes_tokens() {
        let challenges = Challenges::new();
        challenges.insert("tok".to_owned(), "tok.thumb".to_owned());
        assert_eq!(
            challenges.key_authorization("tok").as_deref(),
            Some("tok.thumb")
        );
        challenges.remove("tok");
        assert_eq!(challenges.key_authorization("tok"), None);
    }
}
