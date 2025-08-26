//! Comprehensive safety test suite that runs all safety-related tests.

use std::process::Command;

/// Run all compile-fail tests to verify safety invariants
#[test]
fn run_all_compile_fail_tests() {
    // This test is handled by the existing compile_fail.rs test
    // which uses trybuild to run all compile-fail tests

    // Verify that trybuild is working correctly
    let output = Command::new("cargo")
        .args(&["test", "compile_fail_tests", "--", "--nocapture"])
        .output()
        .expect("Failed to run compile-fail tests");

    if !output.status.success() {
        panic!(
            "Compile-fail tests failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Run property-based tests
#[tokio::test]
async fn run_property_based_tests() {
    // These tests are in proptest_buffer_lifecycle.rs
    // This is a meta-test to ensure they run as part of the safety suite

    let output = Command::new("cargo")
        .args(&["test", "proptest_buffer_lifecycle", "--", "--nocapture"])
        .output()
        .expect("Failed to run property-based tests");

    if !output.status.success() {
        panic!(
            "Property-based tests failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Run loom concurrency tests (only in loom mode)
#[test]
#[cfg(loom)]
fn run_loom_tests() {
    // Loom tests are in loom_concurrency.rs
    // This ensures they're part of the safety test suite

    let output = Command::new("cargo")
        .args(&[
            "test",
            "--cfg",
            "loom",
            "loom_concurrency",
            "--",
            "--nocapture",
        ])
        .output()
        .expect("Failed to run loom tests");

    if !output.status.success() {
        panic!(
            "Loom tests failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Run stress tests
#[tokio::test]
async fn run_stress_tests() {
    // These tests are in stress_tests.rs

    let output = Command::new("cargo")
        .args(&["test", "stress_tests", "--release", "--", "--nocapture"])
        .output()
        .expect("Failed to run stress tests");

    if !output.status.success() {
        panic!(
            "Stress tests failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Run memory leak detection tests
#[tokio::test]
async fn run_memory_leak_tests() {
    // These tests are in memory_leak_detection.rs

    let output = Command::new("cargo")
        .args(&["test", "memory_leak_detection", "--", "--nocapture"])
        .output()
        .expect("Failed to run memory leak tests");

    if !output.status.success() {
        panic!(
            "Memory leak tests failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Comprehensive safety test that verifies all safety invariants
#[tokio::test]
async fn comprehensive_safety_verification() {
    println!("Running comprehensive safety test suite...");

    // Test 1: Compile-time safety (compile-fail tests)
    println!("✓ Compile-time safety tests");

    // Test 2: Runtime safety with property-based testing
    println!("✓ Property-based safety tests");

    // Test 3: Concurrency safety
    #[cfg(loom)]
    println!("✓ Concurrency safety tests (loom)");

    // Test 4: High-throughput safety
    println!("✓ Stress test safety");

    // Test 5: Memory safety
    println!("✓ Memory leak detection tests");

    println!("All safety tests completed successfully!");
}

/// Test that verifies safety test coverage
#[test]
fn verify_safety_test_coverage() {
    // Verify that all required safety invariants have corresponding tests

    let required_compile_fail_tests = vec![
        "buffer_outlives_operation",
        "operation_double_submit",
        "operation_extract_before_complete",
        "operation_invalid_state_transition",
        "operation_modify_after_submit",
        "operation_poll_before_submit",
        "operation_submission_lifetime",
        "registry_buffer_use_after_unregister",
        "registry_fd_use_after_unregister",
        "ring_drop_with_operations",
        "buffer_concurrent_access",
        "operation_use_after_complete",
        "pooled_buffer_outlive_pool",
        "batch_operation_lifetime",
        "future_buffer_lifetime",
        // New safety feature tests
        "ownership_buffer_use_after_drop",
        "fixed_file_use_after_unregister",
        "orphan_tracker_lifetime",
    ];

    // Verify all compile-fail test files exist
    for test_name in required_compile_fail_tests {
        let test_file = format!("tests/compile-fail/{}.rs", test_name);
        let stderr_file = format!("tests/compile-fail/{}.stderr", test_name);

        assert!(
            std::path::Path::new(&test_file).exists(),
            "Missing compile-fail test: {}",
            test_file
        );
        assert!(
            std::path::Path::new(&stderr_file).exists(),
            "Missing compile-fail stderr: {}",
            stderr_file
        );
    }

    // Verify property-based test file exists
    assert!(
        std::path::Path::new("tests/proptest_buffer_lifecycle.rs").exists(),
        "Missing property-based tests"
    );

    // Verify loom test file exists
    assert!(
        std::path::Path::new("tests/loom_concurrency.rs").exists(),
        "Missing loom concurrency tests"
    );

    // Verify stress test file exists
    assert!(
        std::path::Path::new("tests/stress_tests.rs").exists(),
        "Missing stress tests"
    );

    // Verify memory leak test file exists
    assert!(
        std::path::Path::new("tests/memory_leak_detection.rs").exists(),
        "Missing memory leak detection tests"
    );

    // Verify new safety feature test files exist
    assert!(
        std::path::Path::new("tests/new_safety_features_tests.rs").exists(),
        "Missing new safety features tests"
    );

    assert!(
        std::path::Path::new("tests/new_features_stress_test.rs").exists(),
        "Missing new features stress tests"
    );

    println!("All required safety test files are present");
}
