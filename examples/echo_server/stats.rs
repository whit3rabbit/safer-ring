//! Statistics tracking for the echo server.

use std::sync::Arc;
use tokio::sync::Mutex;

/// Runtime statistics for the echo server.
#[derive(Debug, Default)]
pub struct ServerStats {
    pub connections_accepted: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub active_connections: u64,
}

impl ServerStats {
    /// Increment connection count and active connections atomically.
    pub fn connection_accepted(&mut self) {
        self.connections_accepted += 1;
        self.active_connections += 1;
    }

    /// Decrement active connections.
    pub fn connection_closed(&mut self) {
        self.active_connections = self.active_connections.saturating_sub(1);
    }

    /// Add to bytes received counter.
    pub fn bytes_received(&mut self, bytes: usize) {
        self.bytes_received += bytes as u64;
    }

    /// Add to bytes sent counter.
    pub fn bytes_sent(&mut self, bytes: usize) {
        self.bytes_sent += bytes as u64;
    }
}

/// Start a background task that periodically prints server statistics.
pub fn start_stats_reporter(stats: Arc<Mutex<ServerStats>>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let stats = stats.lock().await;
            println!(
                "Stats: {} connections, {} bytes RX, {} bytes TX, {} active",
                stats.connections_accepted,
                stats.bytes_received,
                stats.bytes_sent,
                stats.active_connections
            );
        }
    });
}
