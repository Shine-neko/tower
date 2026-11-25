//! When the symbol index is written back to the host (`index/put`).
//!
//! The index is persisted whole, as the warm-start cache of the next run:
//! writing it after every file made the initial indexing of a workspace
//! quadratic (each of N files sends an index of up to N files). It is written
//! once enough changes accumulate, once they are old enough, and whenever the
//! extension has caught up with the host's events or shuts down. A change not
//! yet persisted is lost only if the process dies, and the next run reindexes
//! the files the host delivers anyway.
//!
//! The decision is taken inside a request turn, before its response, so that
//! the `index/put` HostCall cannot interleave with the host's next request
//! (see the protocol hazards in AGENTS.md).

use std::time::{Duration, Instant};

/// Changes after which the index is persisted, however recent.
pub const MAX_UNPERSISTED: u64 = 512;
/// Age of the oldest unpersisted change after which the index is persisted.
pub const MAX_DELAY: Duration = Duration::from_secs(2);

/// When the index was last persisted.
#[derive(Debug)]
pub struct Persistence {
    last: Instant,
}

impl Persistence {
    #[must_use]
    pub fn new(now: Instant) -> Self {
        Self { last: now }
    }

    /// Whether to persist `changes` now; `settled` when the extension has
    /// processed every event the host delivered, or is shutting down.
    #[must_use]
    pub fn due(&self, changes: u64, now: Instant, settled: bool) -> bool {
        changes > 0
            && (settled
                || changes >= MAX_UNPERSISTED
                || now.saturating_duration_since(self.last) >= MAX_DELAY)
    }

    pub fn persisted(&mut self, now: Instant) {
        self.last = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_nothing_unchanged() {
        let now = Instant::now();
        let persistence = Persistence::new(now);
        assert!(!persistence.due(0, now + MAX_DELAY * 10, true));
    }

    #[test]
    fn batches_changes_until_enough_or_old_enough() {
        let start = Instant::now();
        let mut persistence = Persistence::new(start);
        assert!(!persistence.due(1, start, false));
        assert!(!persistence.due(MAX_UNPERSISTED - 1, start, false));
        assert!(persistence.due(MAX_UNPERSISTED, start, false));
        assert!(persistence.due(1, start + MAX_DELAY, false));
        persistence.persisted(start + MAX_DELAY);
        assert!(!persistence.due(1, start + MAX_DELAY, false));
    }

    #[test]
    fn persists_once_caught_up_or_shutting_down() {
        let now = Instant::now();
        assert!(Persistence::new(now).due(1, now, true));
    }
}
