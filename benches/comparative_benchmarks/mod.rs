//! Comparative performance benchmarks for safer-ring vs raw io_uring.
//!
//! This module provides comprehensive performance comparisons with system information logging,
//! statistical analysis, and detailed overhead reporting.

pub mod analysis;
pub mod config;
pub mod file_benchmarks;
pub mod network_benchmarks;
pub mod raw_io_uring;
pub mod stats;
pub mod system_info;
pub mod utils;