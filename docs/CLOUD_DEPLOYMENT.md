# Cloud Deployment Guide

This guide provides comprehensive instructions for deploying applications using safer-ring in cloud environments, where io_uring support may be limited or disabled by default.

## Overview

Cloud platforms often restrict io_uring for security reasons, but safer-ring is designed to gracefully fall back to traditional I/O mechanisms while maintaining API compatibility. This guide covers deployment strategies for major cloud providers and containerized environments.

## Quick Reference

| Platform | io_uring Support | Configuration Required | Fallback Available |
|----------|------------------|------------------------|-------------------|
| AWS EC2 | ✅ Yes (Linux 5.1+) | Kernel version check | ✅ epoll |
| AWS Lambda | ❌ No | None needed | ✅ stub |
| Google Cloud Run | ❌ No | None needed | ✅ stub |
| Google GKE | ⚠️ Restricted | Security context | ✅ epoll |
| Azure Container Instances | ❌ No | None needed | ✅ stub |
| Azure AKS | ⚠️ Restricted | Pod security | ✅ epoll |
| Docker | ⚠️ Restricted | --cap-add or --privileged | ✅ epoll |
| Kubernetes | ⚠️ Restricted | seccomp/AppArmor config | ✅ epoll |

## Platform-Specific Deployment

### AWS

#### AWS EC2
```bash
# Check io_uring availability
uname -r  # Should be >= 5.1

# Install application
cargo install your-app

# Run with automatic detection
./your-app  # safer-ring auto-detects best backend
```

**Performance Expectations on EC2:**
- io_uring available: 2-4x performance improvement
- Kernel 5.4+: Full feature support
- Kernel 5.1-5.3: Basic io_uring support

#### AWS Lambda
```rust
// Lambda deployment - io_uring not available
use safer_ring::{Runtime, Backend};

#[tokio::main]
async fn lambda_handler(event: Value) -> Result<Value, Error> {
    // safer-ring automatically uses stub backend in Lambda
    let runtime = Runtime::auto_detect()?;
    assert_eq!(runtime.backend(), Backend::Stub);
    
    // Your application code - same API regardless of backend
    let ring = Ring::new(32)?;  // Works with stub backend
    // ... rest of application
}
```

#### AWS App Runner
```dockerfile
FROM public.ecr.aws/lambda/provided:al2-x86_64

# App Runner uses Amazon Linux 2 - limited io_uring
COPY target/lambda/your-app ./

# safer-ring will auto-detect and fall back appropriately
CMD ["./your-app"]
```

### Google Cloud

#### Google Cloud Run
```dockerfile
# Cloud Run - no io_uring support
FROM gcr.io/distroless/cc-debian11

COPY target/release/your-app /app

# safer-ring automatically uses stub backend
EXPOSE 8080
CMD ["/app"]
```

#### Google Kubernetes Engine (GKE)
```yaml
# Enable io_uring in GKE with security context
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
spec:
  template:
    spec:
      containers:
      - name: app
        image: your-app:latest
        securityContext:
          # Required for io_uring access
          capabilities:
            add:
              - SYS_ADMIN
          # OR use privileged mode (less secure)
          # privileged: true
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
```

**Alternative GKE Configuration (Recommended):**
```yaml
# Use custom seccomp profile for better security
apiVersion: v1
kind: Pod
metadata:
  annotations:
    seccomp.security.alpha.kubernetes.io/pod: localhost/io-uring-profile
spec:
  containers:
  - name: app
    image: your-app:latest
```

Create custom seccomp profile:
```json
{
  "defaultAction": "SCMP_ACT_ERRNO",
  "architectures": ["SCMP_ARCH_X86_64"],
  "syscalls": [
    {
      "names": ["io_uring_setup", "io_uring_enter", "io_uring_register"],
      "action": "SCMP_ACT_ALLOW"
    }
  ]
}
```

### Microsoft Azure

#### Azure Container Instances
```yaml
# ACI deployment - no io_uring support
apiVersion: '2021-09-01'
location: eastus
name: safer-ring-app
properties:
  containers:
  - name: app
    properties:
      image: your-app:latest
      resources:
        requests:
          cpu: 1
          memoryInGB: 1.5
      ports:
      - protocol: TCP
        port: 80
  restartPolicy: Always
  osType: Linux
```

#### Azure Kubernetes Service (AKS)
```yaml
# AKS with io_uring support
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
spec:
  template:
    spec:
      containers:
      - name: app
        image: your-app:latest
        securityContext:
          privileged: true  # Required for io_uring
        # OR use more specific capabilities
        # securityContext:
        #   capabilities:
        #     add: ["SYS_ADMIN"]
```

## Container Deployment

### Docker

#### Basic Docker Deployment
```dockerfile
FROM ubuntu:22.04

# Ubuntu 22.04 has kernel 5.15 - full io_uring support
RUN apt-get update && apt-get install -y \
    your-app-dependencies

COPY target/release/your-app /usr/local/bin/

# safer-ring will auto-detect capabilities
CMD ["your-app"]
```

#### Docker with io_uring Enabled
```bash
# Run with SYS_ADMIN capability (recommended)
docker run --cap-add=SYS_ADMIN your-app:latest

# OR run in privileged mode (less secure)
docker run --privileged your-app:latest

# Check what backend is being used
docker run --cap-add=SYS_ADMIN your-app:latest --show-backend
```

#### Docker Compose
```yaml
version: '3.8'
services:
  app:
    image: your-app:latest
    cap_add:
      - SYS_ADMIN
    # Alternative: privileged: true
    ports:
      - "8080:8080"
    environment:
      - SAFER_RING_LOG_LEVEL=info
```

### Kubernetes

#### Standard Deployment
```yaml
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
        image: your-app:latest
        ports:
        - containerPort: 8080
        securityContext:
          capabilities:
            add:
              - SYS_ADMIN
        env:
        - name: SAFER_RING_BACKEND
          value: "auto"  # Let safer-ring choose best backend
        resources:
          requests:
            cpu: 200m
            memory: 256Mi
          limits:
            cpu: 500m
            memory: 512Mi
```

#### Pod Security Standards Compatible
```yaml
# For environments with restricted pod security
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
spec:
  template:
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
      containers:
      - name: app
        image: your-app:latest
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
              - ALL
        # safer-ring will use epoll/stub backend automatically
```

## Performance Optimization by Environment

### High-Performance Environments
For environments where io_uring is available:

```rust
use safer_ring::{SaferRingConfig, Runtime};

// Detect optimal configuration
let config = SaferRingConfig::auto_detect()?;
let runtime = Runtime::auto_detect()?;

match runtime.backend() {
    Backend::IoUring => {
        println!("🚀 High-performance mode enabled");
        // Configure for maximum throughput
        let config = SaferRingConfig::high_throughput();
    }
    Backend::Epoll => {
        println!("⚡ Standard performance mode");
        // Configure for reliable operation
        let config = SaferRingConfig::default();
    }
    Backend::Stub => {
        println!("🔧 Compatibility mode");
        // Configure for minimal resource usage
        let config = SaferRingConfig::low_resource();
    }
}
```

### Serverless Environments
For AWS Lambda, Google Cloud Functions, etc.:

```rust
// Optimized for serverless cold starts
use safer_ring::{SaferRingConfig, Ring};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use minimal configuration for fast startup
    let config = SaferRingConfig::serverless();
    let ring = Ring::with_config(config)?;
    
    // Application logic here - same regardless of backend
    Ok(())
}
```

## Monitoring and Observability

### Health Checks
```rust
use safer_ring::{Runtime, is_io_uring_available};

// Health check endpoint
async fn health_check() -> impl Reply {
    let runtime = Runtime::auto_detect().unwrap();
    let status = json!({
        "backend": format!("{:?}", runtime.backend()),
        "io_uring_available": is_io_uring_available(),
        "performance_multiplier": runtime.backend().performance_multiplier(),
        "environment": {
            "cpu_count": runtime.environment().cpu_count,
            "cloud_environment": runtime.is_cloud_environment(),
        }
    });
    
    warp::reply::json(&status)
}
```

### Metrics Collection
```rust
// Prometheus metrics example
use prometheus::{Counter, Histogram, register_counter, register_histogram};

lazy_static! {
    static ref BACKEND_USAGE: Counter = register_counter!(
        "safer_ring_backend_total",
        "Total backend usage by type"
    ).unwrap();
    
    static ref OPERATION_DURATION: Histogram = register_histogram!(
        "safer_ring_operation_duration_seconds",
        "Operation duration by backend"
    ).unwrap();
}

// In your application
let runtime = Runtime::auto_detect()?;
BACKEND_USAGE.inc();

let timer = OPERATION_DURATION.start_timer();
// ... perform operation
timer.observe_duration();
```

## Troubleshooting

### Common Issues

#### io_uring Not Available
```bash
# Check kernel version
uname -r

# Check if io_uring is compiled in
zgrep IO_URING /proc/config.gz

# Check for restrictions
dmesg | grep -i "io_uring"

# Test with strace
strace -e io_uring_setup your-app 2>&1 | grep -i "operation not permitted"
```

#### Container Permission Issues
```bash
# Check current capabilities
capsh --print

# Test different capability sets
docker run --cap-add=SYS_ADMIN your-app:latest
docker run --cap-add=SYS_RESOURCE your-app:latest
```

#### Kubernetes Security Policies
```bash
# Check pod security policy
kubectl get psp

# Check security context constraints (OpenShift)
oc get scc

# Verify container security context
kubectl describe pod your-pod-name
```

### Performance Diagnostics

#### Backend Detection
```rust
// Add to your application startup
let runtime = Runtime::auto_detect()?;
println!("Backend: {}", runtime.backend().description());
println!("Performance guidance:");
for tip in runtime.performance_guidance() {
    println!("  • {}", tip);
}
```

#### Performance Comparison
```rust
// Benchmark different backends
use std::time::Instant;

async fn benchmark_backend() {
    let runtime = Runtime::auto_detect().unwrap();
    println!("Testing {} backend", runtime.backend().description());
    
    let start = Instant::now();
    
    // Perform your typical workload
    for _ in 0..1000 {
        // Your I/O operations here
    }
    
    let duration = start.elapsed();
    let multiplier = runtime.backend().performance_multiplier();
    println!("Actual: {:?}, Expected multiplier: {}x", duration, multiplier);
}
```

## Migration Checklist

When migrating an existing application to safer-ring:

### Pre-Migration
- [ ] Check target platform io_uring support
- [ ] Review container security requirements
- [ ] Identify performance-critical operations
- [ ] Plan rollback strategy

### Migration
- [ ] Replace traditional I/O with safer-ring APIs
- [ ] Add runtime detection to startup code
- [ ] Configure appropriate backend selection
- [ ] Update container security contexts
- [ ] Add health checks and monitoring

### Post-Migration
- [ ] Verify performance improvements
- [ ] Monitor error rates and fallback usage
- [ ] Update documentation and runbooks
- [ ] Train operations team on new metrics

## Support Matrix

| Cloud Provider | Service | io_uring Support | Recommended Config |
|---------------|---------|------------------|-------------------|
| AWS | EC2 | ✅ Full | Auto-detect |
| AWS | ECS | ⚠️ Capability required | `--cap-add SYS_ADMIN` |
| AWS | Fargate | ❌ None | Stub backend |
| AWS | Lambda | ❌ None | Stub backend |
| Google | GCE | ✅ Full | Auto-detect |
| Google | GKE | ⚠️ Security context | Custom seccomp |
| Google | Cloud Run | ❌ None | Stub backend |
| Azure | VM | ✅ Full | Auto-detect |
| Azure | AKS | ⚠️ Privileged required | `privileged: true` |
| Azure | ACI | ❌ None | Stub backend |

For the most up-to-date support matrix, see the [platform compatibility documentation](./PLATFORM_COMPATIBILITY.md).