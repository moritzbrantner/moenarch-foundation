use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// A shareable token for cooperative cancellation.
///
/// Cancellation is sticky: after any clone requests cancellation, all clones
/// continue to observe the cancelled state.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates an uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    ///
    /// Repeated requests are idempotent.
    pub fn cancel(&self) {
        // SeqCst keeps cancellation observation straightforward across threads:
        // every load and store participates in one global order.
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns whether cancellation has been requested by any clone.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}
