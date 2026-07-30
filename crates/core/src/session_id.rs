//! Process-unique identifier for one session.

use std::sync::atomic::{AtomicU64, Ordering};

/// Identifies one session for the lifetime of the process that ran it.
///
/// Sessions are never persisted or resumed across process runs (there is no
/// `Idle` state to resume into — see [`crate::SessionState`]), so this only
/// needs to be unique within one process, not globally. Generated from a
/// monotonic counter rather than a random or time-based scheme, keeping
/// `core` free of any dependency beyond `thiserror`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(u64);

impl SessionId {
    /// Allocates a new id, distinct from every other id created by this
    /// process so far.
    #[must_use]
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// The underlying counter value.
    #[must_use]
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::SessionId;

    #[test]
    fn successive_ids_differ() {
        let first = SessionId::new();
        let second = SessionId::new();
        assert_ne!(first, second);
    }
}
