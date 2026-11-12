//! Deterministic work counters for the complexity contracts in `.performance/contract.json`.
//!
//! The counters are recorded only with the test-only `work-counters` feature; every call site
//! is a no-op otherwise. They count Foundation-owned search composition only: generic top-K
//! retention work stays with `search-kernels`.

/// Counted search operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Counter {
    /// Exact similarity (metric distance) evaluations of a query against one record vector.
    SimilarityEvaluations,
    /// Vector components read by those evaluations: `D` per evaluation.
    SimilarityComponents,
    /// Hyperplane projection terms computed for LSH signatures: `B * D` per signature, for
    /// indexed records and queries alike.
    ProjectionTerms,
    /// LSH probe signatures looked up in the bucket map, whether the bucket exists or not.
    SignatureProbes,
    /// LSH bucket entries traversed before the exact-score cap stops the probe.
    BucketEntryVisits,
}

const COUNTERS: usize = 5;

#[cfg(feature = "work-counters")]
std::thread_local! {
    static WORK: std::cell::Cell<[u64; COUNTERS]> = const { std::cell::Cell::new([0; COUNTERS]) };
}

#[inline(always)]
pub(crate) fn add(counter: Counter, amount: usize) {
    #[cfg(feature = "work-counters")]
    WORK.with(|work| {
        let mut values = work.get();
        values[counter as usize] += amount as u64;
        work.set(values);
    });
    #[cfg(not(feature = "work-counters"))]
    let _ = (counter, amount, COUNTERS);
}

/// Value of one counter on this thread since the last [`reset`].
#[cfg(feature = "work-counters")]
pub fn get(counter: Counter) -> u64 {
    WORK.with(std::cell::Cell::get)[counter as usize]
}

/// Resets every counter on this thread.
#[cfg(feature = "work-counters")]
pub fn reset() {
    WORK.with(|work| work.set([0; COUNTERS]));
}
