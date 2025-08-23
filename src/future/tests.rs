//! Tests for future implementations.

use crate::future::waker::WakerRegistry;
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
fn waker_registry_basic_functionality() {
    let registry = WakerRegistry::new();
    let (waker, woken) = create_test_waker();

    // Register waker
    registry.register_waker(1, waker);
    assert_eq!(registry.waker_count(), 1);

    // Wake operation
    let was_woken = registry.wake_operation(1);
    assert!(was_woken);
    assert_eq!(registry.waker_count(), 0);
    assert!(*woken.lock().unwrap());
}

#[test]
fn waker_registry_cleanup() {
    let registry = WakerRegistry::new();
    let (waker, woken) = create_test_waker();

    // Register and remove without waking
    registry.register_waker(1, waker);
    let was_removed = registry.remove_waker(1);
    assert!(was_removed);
    assert_eq!(registry.waker_count(), 0);
    assert!(!*woken.lock().unwrap());
}

// Note: Full integration tests for the futures would require a working Ring implementation
// and actual io_uring operations, which are tested in the integration test suite.
// These unit tests focus on the waker registry functionality that can be tested in isolation.
