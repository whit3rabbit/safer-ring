# Setup Examples and Quick Start

This document provides ready-to-use setup examples for deploying safer-ring applications across different environments and platforms.

## Quick Start Templates

### 1. Basic Application Template

```rust
// src/main.rs
use safer_ring::{Ring, PinnedBuffer, Runtime, SaferRingConfig};
use std::os::unix::io::AsRawFd;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Detect optimal configuration for current platform
    let runtime = Runtime::auto_detect()?;
    println!("Using {} backend", runtime.backend().description());
    
    // Create ring with auto-detected configuration
    let config = SaferRingConfig::auto_detect()?;
    let ring = Ring::with_config(config)?;
    
    // Your application logic here
    run_application(ring).await
}

async fn run_application(ring: Ring<'_>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Application started with {} orphans tracked", ring.orphan_count());
    
    // Example: Echo server
    let listener = std::net::TcpListener::bind("127.0.0.1:8080")?;
    let listener_fd = listener.as_raw_fd();
    println!("Listening on 127.0.0.1:8080");
    
    loop {
        let client_fd = ring.accept(listener_fd).await?;
        println!("Accepted connection: {}", client_fd);
        
        // Handle client connection
        tokio::spawn(handle_client(client_fd));
    }
}

async fn handle_client(client_fd: i32) -> Result<(), Box<dyn std::error::Error>> {
    // Note: This is a simplified example
    // In a real application, you'd need to properly manage the ring lifecycle
    Ok(())
}
```

### 2. High-Performance Server Template

```rust
// src/server.rs
use safer_ring::{Ring, PinnedBuffer, Registry, FixedFile, SaferRingConfig};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct HighPerformanceServer {
    ring: Arc<Ring<'static>>,
    registry: Arc<Registry<'static>>,
    connection_semaphore: Arc<Semaphore>,
    log_file: FixedFile,
}

impl HighPerformanceServer {
    pub async fn new(max_connections: usize) -> Result<Self, Box<dyn std::error::Error>> {
        // Configure for high throughput
        let config = SaferRingConfig::high_throughput()
            .with_entries(1024)  // Large ring for high concurrency
            .with_batch_size(64);  // Process multiple operations at once
            
        let ring = Arc::new(Ring::with_config(config)?);
        
        // Set up registry for frequently used files
        let mut registry = Registry::new();
        let log_fd = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("server.log")?
            .as_raw_fd();
            
        let fixed_files = registry.register_fixed_files(vec![log_fd])?;
        let log_file = fixed_files[0].clone();
        
        Ok(Self {
            ring,
            registry: Arc::new(registry),
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            log_file,
        })
    }
    
    pub async fn start(&self, bind_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = std::net::TcpListener::bind(bind_addr)?;
        let listener_fd = listener.as_raw_fd();
        
        println!("High-performance server listening on {}", bind_addr);
        
        loop {
            // Rate limiting with semaphore
            let permit = self.connection_semaphore.clone().acquire_owned().await?;
            
            let client_fd = self.ring.accept(listener_fd).await?;
            
            // Spawn task to handle client
            let ring = self.ring.clone();
            let log_file = self.log_file.clone();
            
            tokio::spawn(async move {
                let _permit = permit;  // Hold permit until task completes
                
                if let Err(e) = Self::handle_connection(ring, client_fd, log_file).await {
                    eprintln!("Connection error: {}", e);
                }
            });
        }
    }
    
    async fn handle_connection(
        ring: Arc<Ring<'_>>,
        client_fd: i32,
        log_file: FixedFile,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = PinnedBuffer::with_capacity(8192);
        
        // Read request
        let (bytes_read, buffer) = ring.recv(client_fd, buffer.as_mut_slice()).await?;
        
        if bytes_read == 0 {
            return Ok(());  // Connection closed
        }
        
        // Process request (echo in this example)
        let response = format!("Echo: {}", 
            String::from_utf8_lossy(&buffer.as_slice()[..bytes_read]));
        
        // Send response
        let response_buffer = PinnedBuffer::from_slice(response.as_bytes());
        let (bytes_sent, _) = ring.send(client_fd, response_buffer.as_slice()).await?;
        
        // Log to fixed file for performance
        let log_entry = format!("Handled {} bytes -> {} bytes\n", bytes_read, bytes_sent);
        let log_buffer = PinnedBuffer::from_slice(log_entry.as_bytes());
        let _ = ring.write_fixed(&log_file, log_buffer.as_slice()).await;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = HighPerformanceServer::new(1000).await?;
    server.start("0.0.0.0:8080").await
}
```

## Docker Deployment Examples

### 3. Basic Docker Setup

```dockerfile
# Dockerfile
FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy application binary
COPY target/release/safer-ring-app /usr/local/bin/app

# Create non-root user
RUN useradd -r -s /bin/false appuser
USER appuser

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

# Run application
CMD ["app"]
```

```yaml
# docker-compose.yml
version: '3.8'

services:
  safer-ring-app:
    build: .
    ports:
      - "8080:8080"
    cap_add:
      - SYS_ADMIN  # Required for io_uring
    environment:
      - RUST_LOG=info
      - SAFER_RING_LOG_LEVEL=debug
    volumes:
      - app_logs:/app/logs
    restart: unless-stopped
    
    # Health check
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  # Optional: Monitoring
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml

volumes:
  app_logs:
```

### 4. Multi-Stage Docker Build

```dockerfile
# Multi-stage build for smaller production images
FROM rust:1.70 AS builder

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Build application
COPY src ./src
RUN touch src/main.rs  # Force rebuild of main.rs
RUN cargo build --release --bin safer-ring-app

# Production stage
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false appuser

COPY --from=builder /app/target/release/safer-ring-app /usr/local/bin/app

# Create directories with proper permissions  
RUN mkdir -p /app/logs && chown appuser:appuser /app/logs

USER appuser
WORKDIR /app

EXPOSE 8080
CMD ["app"]
```

## Kubernetes Deployment Examples

### 5. Basic Kubernetes Deployment

```yaml
# k8s-basic.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
  labels:
    app: safer-ring-app
spec:
  replicas: 3
  selector:
    matchLabels:
      app: safer-ring-app
  template:
    metadata:
      labels:
        app: safer-ring-app
    spec:
      containers:
      - name: app
        image: your-registry/safer-ring-app:latest
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: RUST_LOG
          value: "info"
        - name: SAFER_RING_BACKEND
          value: "auto"
        
        # Required for io_uring
        securityContext:
          capabilities:
            add:
              - SYS_ADMIN
        
        # Resource limits
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 500m
            memory: 512Mi
        
        # Health checks
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
          
      # Restart policy
      restartPolicy: Always
      
---
apiVersion: v1
kind: Service
metadata:
  name: safer-ring-service
spec:
  selector:
    app: safer-ring-app
  ports:
  - port: 80
    targetPort: 8080
    name: http
  type: LoadBalancer

---
# Optional: Horizontal Pod Autoscaler
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: safer-ring-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: safer-ring-app
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

### 6. Production Kubernetes Setup

```yaml
# k8s-production.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: safer-ring-config
data:
  config.toml: |
    [server]
    bind_address = "0.0.0.0:8080"
    max_connections = 1000
    
    [safer_ring]
    backend = "auto"
    ring_entries = 512
    batch_size = 32
    
    [logging]
    level = "info"
    structured = true

---
apiVersion: v1
kind: Secret
metadata:
  name: safer-ring-secrets
type: Opaque
stringData:
  database_url: "postgresql://user:pass@db:5432/mydb"

---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
  labels:
    app: safer-ring-app
    version: v1.0.0
spec:
  replicas: 5
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 2
      maxUnavailable: 1
      
  selector:
    matchLabels:
      app: safer-ring-app
      
  template:
    metadata:
      labels:
        app: safer-ring-app
        version: v1.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        
    spec:
      # Security context
      securityContext:
        runAsNonRoot: true
        runAsUser: 10001
        fsGroup: 10001
        
      containers:
      - name: app
        image: your-registry/safer-ring-app:v1.0.0
        imagePullPolicy: Always
        
        ports:
        - containerPort: 8080
          name: http
          protocol: TCP
        - containerPort: 9090
          name: metrics
          protocol: TCP
          
        env:
        - name: CONFIG_PATH
          value: /etc/config/config.toml
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: safer-ring-secrets
              key: database_url
        
        # io_uring support
        securityContext:
          allowPrivilegeEscalation: false
          capabilities:
            add:
              - SYS_ADMIN
            drop:
              - ALL
          readOnlyRootFilesystem: true
          
        # Mounts
        volumeMounts:
        - name: config
          mountPath: /etc/config
          readOnly: true
        - name: tmp
          mountPath: /tmp
        - name: logs
          mountPath: /app/logs
          
        # Resource management
        resources:
          requests:
            cpu: 200m
            memory: 256Mi
          limits:
            cpu: 1000m
            memory: 1Gi
            
        # Health checks
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 60
          periodSeconds: 30
          timeoutSeconds: 5
          failureThreshold: 3
          
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
          
        # Startup probe for slow initialization
        startupProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 0
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 30
          
      volumes:
      - name: config
        configMap:
          name: safer-ring-config
      - name: tmp
        emptyDir: {}
      - name: logs
        emptyDir: {}
        
      # Affinity rules for better distribution
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - safer-ring-app
              topologyKey: kubernetes.io/hostname

---
# Network policy for security
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: safer-ring-netpol
spec:
  podSelector:
    matchLabels:
      app: safer-ring-app
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: load-balancer
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: database
    ports:
    - protocol: TCP
      port: 5432
  - to: []  # Allow DNS
    ports:
    - protocol: UDP
      port: 53
```

## Cloud Provider Examples

### 7. AWS ECS/Fargate

```json
{
  "family": "safer-ring-task",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "512",
  "memory": "1024",
  "executionRoleArn": "arn:aws:iam::123456789012:role/ecsTaskExecutionRole",
  "taskRoleArn": "arn:aws:iam::123456789012:role/ecsTaskRole",
  "containerDefinitions": [
    {
      "name": "safer-ring-app",
      "image": "123456789012.dkr.ecr.us-east-1.amazonaws.com/safer-ring-app:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "SAFER_RING_BACKEND",
          "value": "auto"
        },
        {
          "name": "AWS_REGION",
          "value": "us-east-1"
        }
      ],
      "secrets": [
        {
          "name": "DATABASE_URL",
          "valueFrom": "arn:aws:secretsmanager:us-east-1:123456789012:secret:prod/database-url"
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/safer-ring-app",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "healthCheck": {
        "command": [
          "CMD-SHELL",
          "curl -f http://localhost:8080/health || exit 1"
        ],
        "interval": 30,
        "timeout": 5,
        "retries": 3,
        "startPeriod": 60
      }
    }
  ]
}
```

### 8. Google Cloud Run

```yaml
# cloudrun.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: safer-ring-app
  annotations:
    run.googleapis.com/ingress: all
    run.googleapis.com/execution-environment: gen2
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "1"
        autoscaling.knative.dev/maxScale: "100"
        run.googleapis.com/cpu-throttling: "false"
    spec:
      containerConcurrency: 100
      timeoutSeconds: 300
      containers:
      - image: gcr.io/PROJECT_ID/safer-ring-app:latest
        ports:
        - containerPort: 8080
        env:
        - name: SAFER_RING_BACKEND
          value: "auto"  # Will use stub backend in Cloud Run
        - name: PORT
          value: "8080"
        resources:
          limits:
            cpu: "2"
            memory: "2Gi"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
```

```bash
# Deploy to Cloud Run
gcloud run deploy safer-ring-app \
  --image gcr.io/PROJECT_ID/safer-ring-app:latest \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --max-instances 100 \
  --cpu 2 \
  --memory 2Gi \
  --concurrency 100 \
  --timeout 300
```

### 9. Azure Container Apps

```yaml
# azure-container-app.yaml
apiVersion: apps/v1beta1
kind: ContainerApp
metadata:
  name: safer-ring-app
spec:
  managedEnvironmentId: /subscriptions/{subscription-id}/resourceGroups/{rg}/providers/Microsoft.App/managedEnvironments/{env-name}
  configuration:
    secrets:
    - name: database-url
      value: "postgresql://user:pass@host:5432/db"
    ingress:
      external: true
      targetPort: 8080
      traffic:
      - weight: 100
        latestRevision: true
  template:
    containers:
    - image: yourregistry.azurecr.io/safer-ring-app:latest
      name: safer-ring-app
      env:
      - name: SAFER_RING_BACKEND
        value: "auto"
      - name: DATABASE_URL
        secretRef: database-url
      resources:
        cpu: 1.0
        memory: 2Gi
      probes:
      - type: Liveness
        httpGet:
          path: /health
          port: 8080
        initialDelaySeconds: 30
        periodSeconds: 10
    scale:
      minReplicas: 1
      maxReplicas: 20
      rules:
      - name: http-rule
        http:
          metadata:
            concurrentRequests: "10"
```

## Development Environment Setup

### 10. Local Development with Docker Compose

```yaml
# docker-compose.dev.yml
version: '3.8'

services:
  app:
    build:
      context: .
      dockerfile: Dockerfile.dev
    volumes:
      - .:/app
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/app/target
    ports:
      - "8080:8080"
      - "9090:9090"  # Metrics
    cap_add:
      - SYS_ADMIN
    environment:
      - RUST_LOG=debug
      - SAFER_RING_LOG_LEVEL=trace
      - DATABASE_URL=postgresql://dev:dev@postgres:5432/devdb
    depends_on:
      postgres:
        condition: service_healthy
    command: cargo watch -x run

  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: devdb
      POSTGRES_USER: dev
      POSTGRES_PASSWORD: dev
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U dev"]
      interval: 5s
      timeout: 5s
      retries: 5

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.dev.yml:/etc/prometheus/prometheus.yml

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana

volumes:
  postgres_data:
  grafana_data:
  cargo_cache:
  target_cache:
```

```dockerfile
# Dockerfile.dev
FROM rust:1.70

RUN apt-get update && apt-get install -y \
    liburing-dev \
    pkg-config \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-watch for development
RUN cargo install cargo-watch

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build
RUN rm -rf src

EXPOSE 8080 9090
CMD ["cargo", "run"]
```

### 11. VS Code Development Container

```json
// .devcontainer/devcontainer.json
{
  "name": "Safer-Ring Development",
  "dockerComposeFile": [
    "../docker-compose.dev.yml"
  ],
  "service": "app",
  "workspaceFolder": "/app",
  "remoteUser": "root",
  
  "customizations": {
    "vscode": {
      "extensions": [
        "rust-lang.rust-analyzer",
        "serayuzgur.crates",
        "vadimcn.vscode-lldb",
        "ms-vscode.docker",
        "ms-kubernetes-tools.vscode-kubernetes-tools"
      ],
      "settings": {
        "rust-analyzer.cargo.features": "all",
        "rust-analyzer.checkOnSave.command": "clippy"
      }
    }
  },
  
  "postCreateCommand": "cargo build",
  "postStartCommand": "cargo test --lib",
  
  "forwardPorts": [8080, 9090, 5432, 3000, 9091],
  
  "features": {
    "ghcr.io/devcontainers/features/docker-in-docker:2": {},
    "ghcr.io/devcontainers/features/kubectl-helm-minikube:1": {}
  }
}
```

## Monitoring and Observability

### 12. Prometheus Metrics Setup

```rust
// src/metrics.rs
use prometheus::{Counter, Histogram, Gauge, register_counter, register_histogram, register_gauge};
use safer_ring::{Runtime, Backend};

lazy_static::lazy_static! {
    // Backend usage metrics
    pub static ref BACKEND_COUNTER: Counter = register_counter!(
        "safer_ring_backend_total",
        "Total operations by backend type"
    ).unwrap();
    
    // Operation timing
    pub static ref OPERATION_DURATION: Histogram = register_histogram!(
        "safer_ring_operation_duration_seconds",
        "Duration of safer-ring operations"
    ).unwrap();
    
    // Ring statistics
    pub static ref ORPHANED_OPERATIONS: Gauge = register_gauge!(
        "safer_ring_orphaned_operations",
        "Current number of orphaned operations"
    ).unwrap();
    
    // Performance metrics
    pub static ref PERFORMANCE_MULTIPLIER: Gauge = register_gauge!(
        "safer_ring_performance_multiplier",
        "Expected performance multiplier vs baseline"
    ).unwrap();
}

pub fn initialize_metrics() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::auto_detect()?;
    
    // Set static metrics
    let multiplier = runtime.backend().performance_multiplier() as f64;
    PERFORMANCE_MULTIPLIER.set(multiplier);
    
    // Log backend info
    match runtime.backend() {
        Backend::IoUring => BACKEND_COUNTER.inc(),
        Backend::Epoll => BACKEND_COUNTER.inc(), 
        Backend::Stub => BACKEND_COUNTER.inc(),
    }
    
    println!("Metrics initialized for {} backend", runtime.backend().description());
    Ok(())
}

// Middleware to track operation metrics
pub struct MetricsMiddleware;

impl MetricsMiddleware {
    pub fn time_operation<F, T>(&self, operation: F) -> T 
    where 
        F: FnOnce() -> T,
    {
        let _timer = OPERATION_DURATION.start_timer();
        operation()
    }
}
```

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'safer-ring-app'
    static_configs:
      - targets: ['app:9090']
    scrape_interval: 5s
    metrics_path: /metrics

  - job_name: 'kubernetes-pods'
    kubernetes_sd_configs:
    - role: pod
    relabel_configs:
    - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
      action: keep
      regex: true
    - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_port]
      action: replace
      target_label: __address__
      regex: (.+)
      replacement: ${1}:9090
```

## Performance Testing Setup

### 13. Load Testing with K6

```javascript
// load-test.js
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');

export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up
    { duration: '5m', target: 100 },   // Steady state
    { duration: '2m', target: 200 },   // Scale up
    { duration: '5m', target: 200 },   // Peak load
    { duration: '2m', target: 0 },     // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% under 500ms
    errors: ['rate<0.1'],             // Error rate under 10%
  },
};

export default function () {
  const response = http.post('http://localhost:8080/api/data', 
    JSON.stringify({
      message: `Test message ${__VU}-${__ITER}`,
      timestamp: Date.now(),
    }),
    {
      headers: { 'Content-Type': 'application/json' },
    }
  );

  const result = check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 500ms': (r) => r.timings.duration < 500,
    'body contains expected data': (r) => r.json('status') === 'ok',
  });

  errorRate.add(!result);
  sleep(1);
}

// Backend comparison test
export function handleSummary(data) {
  return {
    'summary.json': JSON.stringify(data),
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
  };
}
```

```bash
# Run load tests
k6 run --out influxdb=http://localhost:8086/k6 load-test.js

# Compare backends
SAFER_RING_BACKEND=io_uring k6 run load-test.js
SAFER_RING_BACKEND=epoll k6 run load-test.js
```

## CI/CD Pipeline Examples

### 14. GitHub Actions

```yaml
# .github/workflows/ci.yml
name: CI/CD Pipeline

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  test:
    name: Test Suite
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta]
        
    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: ${{ matrix.rust }}
        override: true
        components: rustfmt, clippy
        
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y liburing-dev pkg-config
        
    - name: Cache cargo registry
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
        
    - name: Format check
      run: cargo fmt --all -- --check
      
    - name: Lint
      run: cargo clippy --all-targets --all-features -- -D warnings
      
    - name: Run tests
      run: |
        # Test with different backends
        cargo test --lib --features io_uring
        SAFER_RING_BACKEND=epoll cargo test --lib
        SAFER_RING_BACKEND=stub cargo test --lib
        
    - name: Run integration tests
      run: cargo test --test '*'
      
    - name: Security audit
      run: |
        cargo install cargo-audit
        cargo audit

  build:
    name: Build and Push Docker Image
    runs-on: ubuntu-latest
    needs: test
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Log in to Container Registry
      uses: docker/login-action@v2
      with:
        registry: ${{ env.REGISTRY }}
        username: ${{ github.actor }}
        password: ${{ secrets.GITHUB_TOKEN }}
        
    - name: Extract metadata
      id: meta
      uses: docker/metadata-action@v4
      with:
        images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
        tags: |
          type=ref,event=branch
          type=sha,prefix={{branch}}-
          type=raw,value=latest,enable={{is_default_branch}}
          
    - name: Build and push Docker image
      uses: docker/build-push-action@v4
      with:
        context: .
        push: true
        tags: ${{ steps.meta.outputs.tags }}
        labels: ${{ steps.meta.outputs.labels }}
        cache-from: type=gha
        cache-to: type=gha,mode=max

  deploy:
    name: Deploy to Staging
    runs-on: ubuntu-latest
    needs: build
    if: github.ref == 'refs/heads/main'
    environment: staging
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Configure kubectl
      uses: azure/k8s-set-context@v3
      with:
        method: kubeconfig
        kubeconfig: ${{ secrets.KUBE_CONFIG }}
        
    - name: Deploy to Kubernetes
      run: |
        kubectl set image deployment/safer-ring-app \
          app=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:main-${{ github.sha }}
        kubectl rollout status deployment/safer-ring-app --timeout=300s
        
    - name: Run smoke tests
      run: |
        kubectl port-forward service/safer-ring-service 8080:80 &
        sleep 10
        curl -f http://localhost:8080/health
        pkill kubectl
```

These examples provide a comprehensive foundation for deploying safer-ring applications across various environments while maintaining optimal performance and safety guarantees. Each example can be adapted to your specific requirements and infrastructure setup.