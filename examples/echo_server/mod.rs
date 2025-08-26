//! Echo server modules for safer-ring demonstration.

pub mod config;
pub mod stats;
pub mod client;

pub use config::ServerConfig;
pub use stats::ServerStats;
pub use client::handle_client;