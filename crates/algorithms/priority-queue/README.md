# moenarch-priority-queue

`moenarch-priority-queue` provides a focused addressable min-priority queue for algorithms that need to update or remove an existing queued item without rebuilding the whole queue.

The initial implementation is an indexed binary heap. Its representation is private so later implementations can be compared without making consumers depend on heap indices or tree layout.

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

- `insert`: `O(log n)` and returns an opaque handle.
- `peek_min`: `O(1)`.
- `pop_min`: `O(log n)`.
- `update_priority`: `O(log n)`.
- `remove`: `O(log n)`.
- handle validation: `O(1)`.

Equal priorities are deterministic and stable by insertion order. Updating an existing item's priority does not change that insertion-order tie breaker. Removing and reinserting an item creates a new insertion order.

Handles use generations so a handle to a removed entry cannot silently mutate a later entry that reuses the same internal slot.

## Evidence model

Tests keep a deliberately simple scan-based reference model independent from the production heap and use property-generated operation sequences to compare observable behavior. Criterion workloads record insert/pop, update-heavy, and mixed mutation patterns for future comparisons with pairing, d-ary, Fibonacci, or other implementations.

The benchmarks are evidence, not a claim that the indexed binary heap is always the fastest choice.
