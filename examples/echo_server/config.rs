//! Server configuration for the echo server example.

/// Configuration parameters for the echo server.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ServerConfig {
    /// Address to bind the server to
    pub bind_address: &'static str,
    /// Size of the io_uring submission queue
    pub ring_size: u32,
    /// Buffer size for each connection
    pub buffer_size: usize,
    /// Maximum concurrent connections (for logging/display only in this example)
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080",
            // Use smaller ring size for example - production might use 1024+
            ring_size: 64,
            // 4KB buffer is reasonable for most echo scenarios
            buffer_size: 4096,
            // Add the missing field
            max_connections: 128,
            // 30 second timeout prevents hanging connections
            connection_timeout_secs: 30,
        }
    }
}

impl ServerConfig {
    /// Extract host and port from bind address for display purposes.
    #[allow(dead_code)]
    pub fn host_port(&self) -> (&str, &str) {
        let parts: Vec<&str> = self.bind_address.split(':').collect();
        (
            parts.first().unwrap_or(&"localhost"),
            parts.get(1).unwrap_or(&"8080"),
        )
    }
}
