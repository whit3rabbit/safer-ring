//! Echo server modules for safer-ring demonstration.

pub mod client;
pub mod config;
pub mod stats;

// Do not re-export handle_client here to avoid conflicts.
pub use config::ServerConfig;
pub use stats::ServerStats;