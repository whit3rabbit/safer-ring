# Running Tests

This project uses Cargo’s built-in test harness and includes several test categories:

- Unit/integration tests under `tests/`
- Linux-only io_uring integration tests (gated by `#[cfg(target_os = "linux")]`)
- Compile-fail tests powered by `trybuild`
- Concurrency tests using `loom` (opt-in via cfg flag)
- Property-based tests using `proptest`

## Prerequisites

- Rust stable toolchain (via rustup)
- Linux is required to execute runtime io_uring tests. On non-Linux platforms, Linux-specific tests are compiled out and alternative behavior is tested where applicable.

## Quick start: run everything (default)

```bash
cargo test
```

Notes:
- On non-Linux, Linux-only tests in files like `tests/network_operations.rs` are skipped via cfg.
- Compile-fail tests (in `tests/compile_fail.rs`) run as part of `cargo test` automatically.

## Useful flags and env vars

- Show all test output:
  ```bash
  cargo test -- --nocapture
  ```
- Backtraces on failures:
  ```bash
  RUST_BACKTRACE=1 cargo test
  ```

## Running specific test groups

- Only a single test file (e.g., network ops):
  ```bash
  cargo test --test network_operations
  ```
  This will only run on Linux due to `#[cfg(target_os = "linux")]`.

- Only compile-fail (trybuild) tests:
  ```bash
  cargo test compile_fail_tests -- --nocapture
  ```
  The harness in `tests/compile_fail.rs`:
  ```rust
  let t = trybuild::TestCases::new();
  t.compile_fail("tests/compile-fail/*.rs");
  ```

## Loom concurrency tests

Loom tests are opt-in and gated behind `#![cfg(loom)]` (see `tests/loom_concurrency.rs`). Enable them by adding a compile-time cfg and targeting the test:

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_concurrency -- --nocapture
```

Optional controls (see Loom docs for details):
- Limit exploration to keep runs fast, e.g.:
  ```bash
  LOOM_MAX_BRANCHES=1 RUSTFLAGS="--cfg loom" cargo test --test loom_concurrency
  ```

## Property-based tests (proptest)

Property tests run by default. To increase the number of cases:
```bash
PROPTEST_CASES=1000 cargo test
```

## Platform notes

- `io_uring`-based tests require Linux. On other platforms, the crate compiles with stubs and some tests validate graceful failure (e.g., `Ring::new()` returning an error).

## Troubleshooting

- If a test appears to hang, re-run with verbose output and backtraces:
  ```bash
  RUST_BACKTRACE=1 cargo test -- --nocapture
  ```
- For long-running async or stress tests, consider filtering by name:
  ```bash
  cargo test stress -- --nocapture
  ```
