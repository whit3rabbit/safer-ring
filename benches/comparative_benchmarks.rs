//! Comparative performance benchmarks for safer-ring vs raw io_uring.
//!
//! Provides comprehensive performance comparisons with system information logging,
//! statistical analysis, and detailed overhead reporting.
//!
//! # Usage
//!
//! ```bash
//! cargo bench --bench comparative_benchmarks
//! ```

use criterion::{criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use std::time::Duration;

mod analysis;
mod config;
mod file_benchmarks;
mod network_benchmarks;
mod raw_io_uring;
mod stats;
mod system_info;
mod utils;

use utils::setup_test_file;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
        .measurement_time(config::MEASUREMENT_TIME)
        .sample_size(config::SAMPLE_SIZE);
    targets =
        file_benchmarks::bench_file_copy_comparison,
        network_benchmarks::bench_network_echo_comparison,
        analysis::bench_detailed_performance_analysis
}

criterion_main!(benches);
