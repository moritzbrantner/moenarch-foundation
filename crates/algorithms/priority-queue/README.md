# moenarch-priority-queue

`moenarch-priority-queue` is a compatibility facade for the addressable priority queue owned by [`rust-kernels`](https://github.com/moritzbrantner/rust-kernels), specifically `collection-kernels::AddressablePriorityQueue`.

The algorithm and data-structure implementation, correctness evidence, and primary benchmarks belong in `rust-kernels`. This package keeps the existing Moenarch package and Rust API names stable while delegating the mechanism across the repository boundary.

## Operations

```rust
use priority_queue::AddressablePriorityQueue;

let mut queue = AddressablePriorityQueue::new();
let slow = queue.insert(10, "slow");
let fast = queue.insert(2, "fast");

queue.update_priority(slow, 1)?;
assert_eq!(queue.pop_min(), Some((1, "slow")));
assert_eq!(queue.pop_min(), Some((2, "fast")));

// Handles are opaque and become invalid after removal.
assert!(queue.update_priority(fast, 0).is_err());
# Ok::<(), priority_queue::InvalidHandle>(())
```

The compatibility facade re-exports:

- `AddressablePriorityQueue`
- `PriorityQueueHandle` as the existing `Handle`
- `InvalidPriorityQueueHandle` as the existing `InvalidHandle`

The operational contract remains unchanged: `insert`, `update_priority`, and `remove` are `O(log n)`, `peek_min` is `O(1)`, equal priorities are deterministic by insertion order, and stale or foreign handles are rejected.

## Ownership boundary

`rust-kernels` owns the mechanism. `moenarch-foundation` retains this facade only to preserve the existing package contract and extraction/provenance inventory. New low-level algorithm work belongs in `rust-kernels`; Moenarch capabilities may consume those kernels rather than grow parallel implementations here.
