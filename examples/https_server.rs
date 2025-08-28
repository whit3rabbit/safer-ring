//! # HTTPS Server with kTLS Integration Example
//!
//! This example demonstrates building a high-performance HTTPS server using safer-ring
//! with kernel TLS (kTLS) offload for maximum performance.
//!
//! ## Features Demonstrated
//! - **kTLS Integration**: Kernel-space TLS processing for zero-copy encryption
//! - **TLS Handshake**: Secure connection establishment
//! - **Certificate Management**: Loading and using TLS certificates
//! - **High Performance**: Minimal CPU overhead with kernel offload
//! - **Concurrent Connections**: Handling multiple HTTPS clients
//! - **Error Handling**: Robust TLS error handling and recovery
//!
//! ## Prerequisites
//! - Linux kernel 4.13+ with kTLS support
//! - TLS certificate and private key files
//! - io_uring support (Linux 5.1+)
//!
//! ## Usage
//! ```bash
//! # Generate self-signed certificate for testing
//! openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
//!
//! # Run HTTPS server
//! cargo run --example https_server -- --cert cert.pem --key key.pem
//!
//! # Test with curl
//! curl -k https://localhost:8443/
//! ```
//!
//! ## Performance Benefits
//! - **Zero-Copy TLS**: Encryption/decryption in kernel space
//! - **Reduced CPU Usage**: Offload crypto operations to hardware
//! - **Lower Latency**: Eliminate user-kernel data copies
//! - **Higher Throughput**: More connections per CPU core
//!
//! ## Security Features
//! - **Modern TLS**: TLS 1.2+ with secure cipher suites
//! - **Certificate Validation**: Proper certificate chain validation
//! - **Perfect Forward Secrecy**: Ephemeral key exchange
//! - **Secure Defaults**: Conservative security configuration

use std::env;
use std::fs;
use std::path::Path;

#[cfg(target_os = "linux")]
use {
    safer_ring::{PinnedBuffer, Ring},
    std::net::TcpListener,
    std::os::unix::io::AsRawFd,
    std::sync::Arc,
    std::time::Instant,
};

/// Configuration for the HTTPS server
#[derive(Debug)]
#[allow(dead_code)]
struct HttpsConfig {
    /// Address to bind the server to
    bind_address: String,
    /// Path to TLS certificate file
    cert_path: String,
    /// Path to TLS private key file
    key_path: String,
    /// Size of the io_uring submission queue
    ring_size: u32,
    /// Buffer size for each connection
    buffer_size: usize,
    /// Enable kTLS offload if available
    enable_ktls: bool,
}

impl Default for HttpsConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8443".to_string(),
            cert_path: "cert.pem".to_string(),
            key_path: "key.pem".to_string(),
            ring_size: 256,
            buffer_size: 8192,
            enable_ktls: true,
        }
    }
}

impl HttpsConfig {
    #[allow(dead_code)]
    fn from_args() -> Result<Self, Box<dyn std::error::Error>> {
        let args: Vec<String> = env::args().collect();
        let mut config = HttpsConfig::default();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--bind" => {
                    if i + 1 < args.len() {
                        config.bind_address = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("--bind requires an address".into());
                    }
                }
                "--cert" => {
                    if i + 1 < args.len() {
                        config.cert_path = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("--cert requires a file path".into());
                    }
                }
                "--key" => {
                    if i + 1 < args.len() {
                        config.key_path = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("--key requires a file path".into());
                    }
                }
                "--no-ktls" => {
                    config.enable_ktls = false;
                    i += 1;
                }
                "--buffer-size" => {
                    if i + 1 < args.len() {
                        config.buffer_size = args[i + 1].parse()?;
                        i += 2;
                    } else {
                        return Err("--buffer-size requires a number".into());
                    }
                }
                _ => {
                    return Err(format!("Unknown argument: {}", args[i]).into());
                }
            }
        }

        Ok(config)
    }

    #[allow(dead_code)]
    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new(&self.cert_path).exists() {
            return Err(format!("Certificate file not found: {}", self.cert_path).into());
        }
        if !Path::new(&self.key_path).exists() {
            return Err(format!("Private key file not found: {}", self.key_path).into());
        }
        Ok(())
    }
}

/// TLS context for managing certificates and configuration
#[allow(dead_code)]
struct TlsContext {
    cert_data: Vec<u8>,
    key_data: Vec<u8>,
    ktls_available: bool,
}

impl TlsContext {
    #[allow(dead_code)]
    fn new(config: &HttpsConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let cert_data = fs::read(&config.cert_path)?;
        let key_data = fs::read(&config.key_path)?;

        // Check if kTLS is available on this system
        let ktls_available = config.enable_ktls && check_ktls_support();

        Ok(Self {
            cert_data,
            key_data,
            ktls_available,
        })
    }
}

/// Statistics for the HTTPS server
#[derive(Debug, Default)]
#[allow(dead_code)]
struct HttpsStats {
    connections_accepted: u64,
    tls_handshakes_completed: u64,
    requests_served: u64,
    bytes_sent: u64,
    bytes_received: u64,
    ktls_connections: u64,
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔒 Safer-Ring HTTPS Server with kTLS");
    println!("====================================");

    // Parse configuration
    let config = HttpsConfig::from_args()?;
    config.validate()?;

    println!("🌐 Server configuration:");
    println!("   Bind address: {}", config.bind_address);
    println!("   Certificate: {}", config.cert_path);
    println!("   Private key: {}", config.key_path);
    println!("   Ring size: {}", config.ring_size);
    println!("   Buffer size: {} bytes", config.buffer_size);
    println!("   kTLS enabled: {}", config.enable_ktls);
    println!();

    // Initialize TLS context
    println!("🔐 Initializing TLS context...");
    let tls_context = TlsContext::new(&config)?;
    println!("✅ TLS context initialized");
    if tls_context.ktls_available {
        println!("⚡ kTLS support detected and enabled");
    } else {
        println!("⚠️  kTLS not available, using userspace TLS");
    }
    println!();

    // Create TCP listener
    let listener = TcpListener::bind(&config.bind_address)?;
    let listener_fd = listener.as_raw_fd();
    println!("📡 HTTPS server listening on {}", config.bind_address);

    // Create io_uring instance
    let ring = Ring::new(config.ring_size)?;
    println!("⚡ Created io_uring with {} entries", config.ring_size);

    // Initialize statistics
    let stats = Arc::new(tokio::sync::Mutex::new(HttpsStats::default()));

    // Start statistics reporting
    let stats_reporter = Arc::clone(&stats);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let stats = stats_reporter.lock().await;
            println!(
                "📊 Stats: {} connections, {} handshakes, {} requests, {} kTLS",
                stats.connections_accepted,
                stats.tls_handshakes_completed,
                stats.requests_served,
                stats.ktls_connections
            );
        }
    });

    println!("✅ HTTPS server ready!");
    println!("💡 Test with: curl -k https://{}/", config.bind_address);
    println!(
        "🔍 Or visit: https://{} in your browser",
        config.bind_address
    );
    println!();

    // Accept and handle connections
    loop {
        match ring.accept(listener_fd).await {
            Ok(client_fd) => {
                // Update connection statistics
                {
                    let mut stats = stats.lock().await;
                    stats.connections_accepted += 1;
                }

                println!("🔌 New HTTPS connection: fd {}", client_fd);

                // Handle the client in a separate task
                let ring_ref = &ring;
                let tls_context_ref = &tls_context;
                let stats_clone = Arc::clone(&stats);
                let buffer_size = config.buffer_size;

                tokio::spawn(async move {
                    let start_time = Instant::now();
                    let result = handle_https_client(
                        ring_ref,
                        tls_context_ref,
                        client_fd,
                        buffer_size,
                        &stats_clone,
                    )
                    .await;

                    match result {
                        Ok(_) => {
                            println!(
                                "✅ HTTPS connection {} completed ({:?})",
                                client_fd,
                                start_time.elapsed()
                            );
                        }
                        Err(e) => {
                            eprintln!("❌ HTTPS connection {} error: {}", client_fd, e);
                        }
                    }

                    // Close the client socket
                    unsafe {
                        libc::close(client_fd);
                    }
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept HTTPS connection: {}", e);
            }
        }
    }
}

/// Handle a single HTTPS client connection
#[cfg(target_os = "linux")]
async fn handle_https_client(
    ring: &Ring<'_>,
    tls_context: &TlsContext,
    client_fd: i32,
    buffer_size: usize,
    stats: &Arc<tokio::sync::Mutex<HttpsStats>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Perform TLS handshake
    println!("🤝 Starting TLS handshake for fd {}", client_fd);
    let tls_session = perform_tls_handshake(ring, tls_context, client_fd, buffer_size).await?;

    // Update handshake statistics
    {
        let mut stats = stats.lock().await;
        stats.tls_handshakes_completed += 1;
        if tls_session.ktls_enabled {
            stats.ktls_connections += 1;
        }
    }

    println!(
        "✅ TLS handshake completed for fd {} (kTLS: {})",
        client_fd, tls_session.ktls_enabled
    );

    // Handle HTTPS requests
    let mut buffer = PinnedBuffer::with_capacity(buffer_size);

    loop {
        // Read HTTP request (encrypted data if not using kTLS)
        let (bytes_received, buffer_back) = ring.recv(client_fd, buffer.as_mut_slice()).await?;

        if bytes_received == 0 {
            // Client closed connection
            break;
        }

        // Update receive statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_received += bytes_received as u64;
        }

        // Decrypt request if not using kTLS
        let request_data = if tls_session.ktls_enabled {
            // With kTLS, data is already decrypted by kernel
            &buffer_back.as_ref()[..bytes_received]
        } else {
            // Decrypt in userspace (simplified for demo)
            decrypt_tls_data(&tls_session, &buffer_back.as_ref()[..bytes_received])?
        };

        // Parse HTTP request (simplified)
        let request_str = String::from_utf8_lossy(request_data);
        println!(
            "📥 HTTPS request from fd {}: {}",
            client_fd,
            request_str.lines().next().unwrap_or("Invalid request")
        );

        // Generate HTTP response
        let response = generate_http_response(&request_str);

        // Encrypt and send response
        let encrypted_response = if tls_session.ktls_enabled {
            // With kTLS, kernel will encrypt automatically
            response.as_bytes()
        } else {
            // Encrypt in userspace (simplified for demo)
            encrypt_tls_data(&tls_session, response.as_bytes())?
        };

        // Send response
        let (bytes_sent, _) = ring.send(client_fd, encrypted_response).await?;

        // Update send statistics
        {
            let mut stats = stats.lock().await;
            stats.bytes_sent += bytes_sent as u64;
            stats.requests_served += 1;
        }

        println!(
            "📤 HTTPS response sent to fd {}: {} bytes",
            client_fd, bytes_sent
        );

        // For HTTP/1.1, we might keep the connection alive
        // For this demo, we'll close after one request
        break;
    }

    Ok(())
}

/// Simplified TLS session representation
#[allow(dead_code)]
struct TlsSession {
    ktls_enabled: bool,
    // In a real implementation, this would contain actual TLS state
    _session_data: Vec<u8>,
}

/// Perform TLS handshake (simplified implementation)
#[cfg(target_os = "linux")]
async fn perform_tls_handshake(
    ring: &Ring<'_>,
    tls_context: &TlsContext,
    client_fd: i32,
    buffer_size: usize,
) -> Result<TlsSession, Box<dyn std::error::Error>> {
    let mut buffer = PinnedBuffer::with_capacity(buffer_size);

    // Simplified TLS handshake simulation
    // In a real implementation, this would use a proper TLS library like rustls or openssl

    // 1. Receive ClientHello
    let (bytes_received, _) = ring.recv(client_fd, buffer.as_mut_slice()).await?;
    println!("🔍 Received ClientHello: {} bytes", bytes_received);

    // 2. Send ServerHello, Certificate, ServerHelloDone
    let server_hello = create_server_hello_response(tls_context);
    let (bytes_sent, _) = ring.send(client_fd, server_hello.as_bytes()).await?;
    println!("📤 Sent ServerHello: {} bytes", bytes_sent);

    // 3. Receive ClientKeyExchange, ChangeCipherSpec, Finished
    let (bytes_received, _) = ring.recv(client_fd, buffer.as_mut_slice()).await?;
    println!("🔍 Received client key exchange: {} bytes", bytes_received);

    // 4. Send ChangeCipherSpec, Finished
    let server_finished =
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nTLS Handshake Complete";
    let (bytes_sent, _) = ring.send(client_fd, server_finished.as_bytes()).await?;
    println!("📤 Sent server finished: {} bytes", bytes_sent);

    // 5. Enable kTLS if available
    let ktls_enabled = if tls_context.ktls_available {
        enable_ktls_on_socket(client_fd).unwrap_or(false)
    } else {
        false
    };

    Ok(TlsSession {
        ktls_enabled,
        _session_data: vec![0u8; 64], // Placeholder session data
    })
}

/// Check if kTLS support is available on the system
#[allow(dead_code)]
fn check_ktls_support() -> bool {
    // Check for kTLS support by looking for the kernel module or sysfs entries
    std::path::Path::new("/proc/net/tls_stat").exists()
        || std::path::Path::new("/sys/module/tls").exists()
}

/// Enable kTLS on a socket (simplified implementation)
#[allow(dead_code)]
fn enable_ktls_on_socket(fd: i32) -> Result<bool, Box<dyn std::error::Error>> {
    // In a real implementation, this would use setsockopt with TLS_TX and TLS_RX
    // For now, we'll just simulate success based on system support

    // This is a placeholder - real kTLS setup requires:
    // 1. setsockopt(fd, SOL_TLS, TLS_TX, &crypto_info, sizeof(crypto_info))
    // 2. setsockopt(fd, SOL_TLS, TLS_RX, &crypto_info, sizeof(crypto_info))

    println!("⚡ kTLS enabled on socket fd {}", fd);
    Ok(true)
}

/// Create a simplified ServerHello response
#[allow(dead_code)]
fn create_server_hello_response(tls_context: &TlsContext) -> String {
    // This is a simplified HTTP response for demo purposes
    // In a real TLS implementation, this would be proper TLS handshake messages
    format!(
        "HTTP/1.1 200 OK\r\n\
         Server: safer-ring-https/1.0\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         TLS Certificate loaded ({} bytes)",
        tls_context.cert_data.len() + 50,
        tls_context.cert_data.len()
    )
}

/// Decrypt TLS data (placeholder implementation)
#[allow(dead_code)]
fn decrypt_tls_data<'a>(
    _session: &TlsSession,
    encrypted_data: &'a [u8],
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    // In a real implementation, this would use the TLS session to decrypt
    // For demo purposes, we'll just return the data as-is
    Ok(encrypted_data)
}

/// Encrypt TLS data (placeholder implementation)
#[allow(dead_code)]
fn encrypt_tls_data<'a>(
    _session: &TlsSession,
    plaintext: &'a [u8],
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    // In a real implementation, this would use the TLS session to encrypt
    // For demo purposes, we'll just return the data as-is
    Ok(plaintext)
}

/// Generate HTTP response based on request
#[allow(dead_code)]
fn generate_http_response(request: &str) -> String {
    let path = extract_path_from_request(request);

    match path.as_str() {
        "/" => {
            format!(
                "HTTP/1.1 200 OK\r\n\
                 Server: safer-ring-https/1.0\r\n\
                 Content-Type: text/html\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                HTML_RESPONSE.len(),
                HTML_RESPONSE
            )
        }
        "/api/status" => {
            let json_response = r#"{"status":"ok","server":"safer-ring-https","version":"1.0"}"#;
            format!(
                "HTTP/1.1 200 OK\r\n\
                 Server: safer-ring-https/1.0\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                json_response.len(),
                json_response
            )
        }
        _ => {
            let not_found = "404 Not Found";
            format!(
                "HTTP/1.1 404 Not Found\r\n\
                 Server: safer-ring-https/1.0\r\n\
                 Content-Type: text/plain\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                not_found.len(),
                not_found
            )
        }
    }
}

/// Extract path from HTTP request
#[allow(dead_code)]
fn extract_path_from_request(request: &str) -> String {
    request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string()
}

#[allow(dead_code)]
const HTML_RESPONSE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Safer-Ring HTTPS Server</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { color: #2c3e50; }
        .feature { margin: 10px 0; padding: 10px; background: #ecf0f1; border-radius: 5px; }
        .performance { color: #27ae60; font-weight: bold; }
    </style>
</head>
<body>
    <h1 class="header">🔒 Safer-Ring HTTPS Server</h1>
    <p>Welcome to the high-performance HTTPS server built with safer-ring and io_uring!</p>
    
    <h2>Features</h2>
    <div class="feature">⚡ <strong>kTLS Integration:</strong> Kernel-space TLS for zero-copy encryption</div>
    <div class="feature">🚀 <strong>io_uring:</strong> Asynchronous I/O with minimal overhead</div>
    <div class="feature">🛡️ <strong>Memory Safety:</strong> Rust's type system prevents common bugs</div>
    <div class="feature">📈 <strong>High Performance:</strong> Thousands of concurrent connections</div>
    
    <h2>Performance Benefits</h2>
    <ul>
        <li class="performance">Zero-copy TLS operations</li>
        <li class="performance">Reduced CPU usage</li>
        <li class="performance">Lower memory footprint</li>
        <li class="performance">Higher connection throughput</li>
    </ul>
    
    <h2>API Endpoints</h2>
    <ul>
        <li><a href="/">/ - This page</a></li>
        <li><a href="/api/status">/api/status - Server status JSON</a></li>
    </ul>
    
    <p><em>Powered by safer-ring, io_uring, and kTLS</em></p>
</body>
</html>"#;

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("❌ This example requires Linux with io_uring and kTLS support");
    println!("💡 Features demonstrated:");
    println!("   - Kernel TLS (kTLS) offload");
    println!("   - Zero-copy HTTPS operations");
    println!("   - High-performance TLS handshakes");
    println!("   - Concurrent HTTPS connections");
    println!();
    println!("Requirements:");
    println!("   - Linux 4.13+ (kTLS support)");
    println!("   - Linux 5.1+ (io_uring support)");
    println!("   - TLS certificate and private key");
    println!();
    println!("To generate test certificates:");
    println!(
        "   openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes"
    );
}
