#![doc = include_str!("../README.md")]

pub use collection_kernels::AddressablePriorityQueue;
pub use collection_kernels::InvalidPriorityQueueHandle as InvalidHandle;
pub use collection_kernels::PriorityQueueHandle as Handle;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_names_preserve_the_existing_surface() {
        let mut queue = AddressablePriorityQueue::new();
        let handle: Handle = queue.insert(2, "slow");
        let _: Result<(), InvalidHandle> = queue.update_priority(handle, 1);
        assert_eq!(queue.pop_min(), Some((1, "slow")));
    }
}
