//! Echo server modules for safer-ring demonstration.

pub mod client;
pub mod config;
pub mod stats;

pub use client::handle_client;
pub use config::ServerConfig;
pub use stats::ServerStats;
