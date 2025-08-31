//! Waker registry for managing async operation notifications.

use std::collections::HashMap;
use std::sync::Mutex;
use std::task::Waker;

/// Shared waker registry for managing async operations.
///
/// This registry allows the ring to wake up futures when their operations
/// complete. Uses `RefCell` for interior mutability since we're in a
/// single-threaded async context with io_uring.
#[derive(Debug, Default)]
pub(crate) struct WakerRegistry {
    /// Map from operation ID to waker
    /// Using Mutex for interior mutability to support Arc sharing
    wakers: Mutex<HashMap<u64, Waker>>,
}

impl WakerRegistry {
    /// Create a new waker registry.
    #[allow(dead_code)] // Used by ring initialization
    pub(crate) fn new() -> Self {
        Self {
            wakers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a waker for an operation ID.
    ///
    /// If a waker already exists for this operation, it will be replaced.
    /// This is safe because futures should only be polled from one task.
    pub(crate) fn register_waker(&self, operation_id: u64, waker: Waker) {
        self.wakers.lock().unwrap().insert(operation_id, waker);
    }

    /// Wake a specific operation and remove its waker.
    ///
    /// Returns `true` if a waker was found and woken, `false` otherwise.
    #[allow(dead_code)] // Used by completion processor
    pub(crate) fn wake_operation(&self, operation_id: u64) -> bool {
        if let Some(waker) = self.wakers.lock().unwrap().remove(&operation_id) {
            waker.wake();
            true
        } else {
            false
        }
    }

    /// Remove a waker without waking it (for cleanup).
    ///
    /// Returns `true` if a waker was removed, `false` if none existed.
    pub(crate) fn remove_waker(&self, operation_id: u64) -> bool {
        self.wakers.lock().unwrap().remove(&operation_id).is_some()
    }

    /// Get the number of registered wakers.
    ///
    /// Useful for debugging and monitoring async operation load.
    #[cfg(test)]
    pub(crate) fn waker_count(&self) -> usize {
        self.wakers.lock().unwrap().len()
    }

    /// Check if any wakers are registered.
    ///
    /// More efficient than checking `waker_count() == 0`.
    #[cfg(test)]
    pub(crate) fn has_wakers(&self) -> bool {
        !self.wakers.lock().unwrap().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::task::{RawWaker, RawWakerVTable, Waker};

    // Helper to create a test waker that tracks wake calls
    fn create_test_waker() -> (Waker, Arc<Mutex<bool>>) {
        let woken = Arc::new(Mutex::new(false));
        let woken_clone = woken.clone();

        let raw_waker = RawWaker::new(
            Arc::into_raw(woken_clone) as *const (),
            &RawWakerVTable::new(
                |data| {
                    let woken = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                    let woken_clone = woken.clone();
                    std::mem::forget(woken);
                    RawWaker::new(
                        Arc::into_raw(woken_clone) as *const (),
                        &RawWakerVTable::new(
                            |_| panic!("clone should not be called"),
                            |data| {
                                let woken = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                                *woken.lock().unwrap() = true;
                            },
                            |data| {
                                let woken = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                                *woken.lock().unwrap() = true;
                            },
                            |data| {
                                let _ = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                            },
                        ),
                    )
                },
                |data| {
                    let woken = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                    *woken.lock().unwrap() = true;
                },
                |data| {
                    let woken = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                    *woken.lock().unwrap() = true;
                },
                |data| {
                    let _ = unsafe { Arc::from_raw(data as *const Mutex<bool>) };
                },
            ),
        );

        (unsafe { Waker::from_raw(raw_waker) }, woken)
    }

    #[test]
    fn new_registry_is_empty() {
        let registry = WakerRegistry::new();
        assert_eq!(registry.waker_count(), 0);
        assert!(!registry.has_wakers());
    }

    #[test]
    fn register_and_wake_waker() {
        let registry = WakerRegistry::new();
        let (waker, woken) = create_test_waker();

        registry.register_waker(1, waker);
        assert_eq!(registry.waker_count(), 1);
        assert!(registry.has_wakers());

        let was_woken = registry.wake_operation(1);
        assert!(was_woken);
        assert_eq!(registry.waker_count(), 0);
        assert!(!registry.has_wakers());

        // Check that the waker was actually called
        assert!(*woken.lock().unwrap());
    }

    #[test]
    fn wake_nonexistent_operation() {
        let registry = WakerRegistry::new();
        let was_woken = registry.wake_operation(999);
        assert!(!was_woken);
    }

    #[test]
    fn remove_waker_without_waking() {
        let registry = WakerRegistry::new();
        let (waker, woken) = create_test_waker();

        registry.register_waker(1, waker);
        assert_eq!(registry.waker_count(), 1);

        let was_removed = registry.remove_waker(1);
        assert!(was_removed);
        assert_eq!(registry.waker_count(), 0);

        // Waker should not have been called
        assert!(!*woken.lock().unwrap());
    }

    #[test]
    fn remove_nonexistent_waker() {
        let registry = WakerRegistry::new();
        let was_removed = registry.remove_waker(999);
        assert!(!was_removed);
    }

    #[test]
    fn replace_existing_waker() {
        let registry = WakerRegistry::new();
        let (waker1, woken1) = create_test_waker();
        let (waker2, woken2) = create_test_waker();

        registry.register_waker(1, waker1);
        registry.register_waker(1, waker2); // Replace the first waker

        assert_eq!(registry.waker_count(), 1);

        registry.wake_operation(1);

        // Only the second waker should have been called
        assert!(!*woken1.lock().unwrap());
        assert!(*woken2.lock().unwrap());
    }
}
