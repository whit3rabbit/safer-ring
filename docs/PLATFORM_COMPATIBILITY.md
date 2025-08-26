# Platform Compatibility Matrix

This document provides comprehensive compatibility information for safer-ring across different platforms, kernel versions, and deployment environments.

## Platform Support Overview

| Platform | Status | io_uring Support | Fallback | Notes |
|----------|--------|------------------|----------|-------|
| Linux x86_64 | ✅ Full | Kernel 5.1+ | epoll | Primary target |
| Linux ARM64 | ✅ Full | Kernel 5.1+ | epoll | Full support |
| Linux ARM32 | ✅ Limited | Kernel 5.1+ | epoll | Basic support |
| macOS | ✅ Compatible | No | stub | Development/testing |
| Windows | ✅ Compatible | No | stub | Development/testing |
| FreeBSD | ✅ Compatible | No | stub | Community supported |

## Linux Kernel Compatibility

### io_uring Feature Timeline

| Kernel Version | Features Available | Safer-Ring Support |
|---------------|-------------------|-------------------|
| 5.0 and below | None | ❌ No io_uring |
| 5.1 | Basic io_uring | ✅ Core operations |
| 5.4 | Improved stability | ✅ Recommended minimum |
| 5.6 | Multi-shot operations | ✅ Enhanced performance |
| 5.10 | Buffer selection | ✅ Advanced features |
| 5.15 | Performance improvements | ✅ Optimal performance |
| 5.19 | Latest features | ✅ All features |
| 6.0+ | Best performance | ✅ Maximum optimization |

### Kernel Version Detection
```rust
use safer_ring::runtime::{Runtime, get_environment_info};

fn check_kernel_compatibility() {
    let env = get_environment_info();
    
    if let Some(kernel) = &env.kernel_version {
        println!("Kernel version: {}", kernel);
        
        // Parse version for compatibility check
        if kernel.starts_with("5.") {
            let minor: u32 = kernel.split('.').nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            
            match minor {
                0 => println!("❌ io_uring not available"),
                1..=3 => println!("⚠️  Basic io_uring support"),
                4..=9 => println!("✅ Good io_uring support"),
                _ => println!("✅ Excellent io_uring support"),
            }
        } else if kernel.starts_with("6.") {
            println!("✅ Latest io_uring features available");
        }
    }
}
```

## Cloud Platform Compatibility

### Amazon Web Services (AWS)

#### EC2 Instances
```rust
// AWS EC2 compatibility check
use safer_ring::{Runtime, Backend};

async fn aws_ec2_setup() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::auto_detect()?;
    
    match runtime.backend() {
        Backend::IoUring => {
            println!("✅ EC2: io_uring available");
            println!("Expected performance: 2-4x improvement");
        }
        Backend::Epoll => {
            println!("⚠️  EC2: Using epoll fallback");
            println!("Check AMI kernel version (recommend Amazon Linux 2022+)");
        }
        Backend::Stub => {
            println!("❌ Unexpected on EC2");
        }
    }
    
    Ok(())
}
```

**EC2 AMI Compatibility:**
- Amazon Linux 2022: ✅ Kernel 5.15+
- Amazon Linux 2: ⚠️ Kernel 4.14 (no io_uring)  
- Ubuntu 20.04 LTS: ✅ Kernel 5.4+
- Ubuntu 22.04 LTS: ✅ Kernel 5.15+
- RHEL 8: ⚠️ Kernel 4.18 (backported io_uring)
- RHEL 9: ✅ Kernel 5.14+

#### ECS/Fargate
```yaml
# ECS Task Definition
family: safer-ring-app
taskRoleArn: arn:aws:iam::123456789012:role/ecsTaskRole
executionRoleArn: arn:aws:iam::123456789012:role/ecsTaskExecutionRole
networkMode: awsvpc
requiresCompatibilities:
  - FARGATE
cpu: 256
memory: 512
containerDefinitions:
- name: app
  image: your-app:latest
  # Fargate: io_uring not available, safer-ring uses stub backend
  environment:
  - name: SAFER_RING_BACKEND
    value: "auto"  # Automatically selects appropriate backend
```

#### Lambda
```rust
// AWS Lambda configuration
use safer_ring::{Ring, SaferRingConfig};

#[tokio::main]
async fn lambda_handler(event: lambda_runtime::LambdaEvent<Value>) -> Result<Value, Error> {
    // Lambda: Always uses stub backend
    let config = SaferRingConfig::serverless();  // Optimized for cold starts
    let ring = Ring::with_config(config)?;
    
    // Same API regardless of backend
    handle_request(&ring, event.payload).await
}
```

### Google Cloud Platform (GCP)

#### Compute Engine
```bash
# Check GCE kernel version
gcloud compute instances describe INSTANCE_NAME --zone=ZONE \
  --format="get(metadata.items[key='kernel-version'].value)"

# Most GCE images have modern kernels with io_uring support
```

#### Google Kubernetes Engine (GKE)
```yaml
# GKE with io_uring support
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
spec:
  template:
    spec:
      nodeSelector:
        # Use Container-Optimized OS with recent kernel
        kubernetes.io/os: linux
      containers:
      - name: app
        image: your-app:latest
        securityContext:
          # Required for io_uring in GKE
          capabilities:
            add:
              - SYS_ADMIN
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
---
# Alternative: Custom seccomp profile (more secure)
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

**GKE Node Images:**
- Container-Optimized OS: ✅ Kernel 5.4+
- Ubuntu: ✅ Kernel 5.4+  
- Windows: ❌ Not applicable

#### Cloud Run
```dockerfile
# Cloud Run deployment
FROM gcr.io/distroless/cc-debian11

COPY target/release/safer-ring-app /app

# Cloud Run: No io_uring, uses stub backend
ENV SAFER_RING_BACKEND=auto
EXPOSE 8080

CMD ["/app"]
```

### Microsoft Azure

#### Virtual Machines
```bash
# Check Azure VM kernel
az vm run-command invoke \
  --resource-group myResourceGroup \
  --name myVM \
  --command-id RunShellScript \
  --scripts "uname -r"
```

**Azure VM Images:**
- Ubuntu 20.04: ✅ Kernel 5.4+
- Ubuntu 22.04: ✅ Kernel 5.15+
- CentOS 8: ⚠️ Limited support
- RHEL 8: ⚠️ Limited support
- Debian 11: ✅ Kernel 5.10+

#### Azure Kubernetes Service (AKS)
```yaml
# AKS deployment with io_uring
apiVersion: apps/v1
kind: Deployment
metadata:
  name: safer-ring-app
spec:
  template:
    spec:
      containers:
      - name: app
        image: myregistry.azurecr.io/safer-ring-app:latest
        securityContext:
          privileged: true  # Required for io_uring in AKS
        resources:
          requests:
            cpu: 200m
            memory: 256Mi
```

#### Container Instances
```yaml
# Azure Container Instances
apiVersion: 2021-09-01
location: eastus
name: safer-ring-aci
properties:
  containers:
  - name: app
    properties:
      image: myregistry.azurecr.io/safer-ring-app:latest
      resources:
        requests:
          cpu: 1
          memoryInGB: 1.5
      # ACI: No io_uring support, uses stub backend
      environmentVariables:
      - name: SAFER_RING_BACKEND
        value: auto
  restartPolicy: Always
  osType: Linux
```

## Container Runtime Compatibility

### Docker

#### Version Support
- Docker 20.10+: ✅ Full support with capabilities
- Docker 19.03+: ✅ Basic support
- Docker 18.09+: ⚠️ Limited support

#### Configuration Examples
```bash
# Enable io_uring with capabilities (recommended)
docker run --cap-add=SYS_ADMIN your-app:latest

# Enable with privileged mode (less secure)
docker run --privileged your-app:latest

# Check what's running
docker run --cap-add=SYS_ADMIN your-app:latest safer-ring-info
```

```dockerfile
# Dockerfile for optimal compatibility
FROM ubuntu:22.04

# Install your application
COPY target/release/your-app /usr/local/bin/

# Runtime detection happens automatically
CMD ["your-app"]
```

### Podman
```bash
# Podman with io_uring
podman run --cap-add=SYS_ADMIN your-app:latest

# Rootless podman (limited io_uring)
podman run --user $(id -u):$(id -g) your-app:latest
```

### containerd/runc
```json
{
  "ociVersion": "1.0.2",
  "process": {
    "capabilities": {
      "bounding": ["CAP_SYS_ADMIN"],
      "effective": ["CAP_SYS_ADMIN"],
      "permitted": ["CAP_SYS_ADMIN"]
    }
  }
}
```

## Kubernetes Distributions

### Distribution Compatibility Matrix

| Distribution | io_uring Support | Configuration Required | Notes |
|-------------|------------------|----------------------|-------|
| Vanilla K8s | ✅ Yes | SecurityContext | Depends on nodes |
| OpenShift | ⚠️ Restricted | SCC modification | Enterprise policies |
| Rancher | ✅ Yes | SecurityContext | Full support |
| k3s | ✅ Yes | SecurityContext | Lightweight |
| MicroK8s | ✅ Yes | SecurityContext | Ubuntu-based |
| EKS | ✅ Yes | SecurityContext | AWS managed |
| GKE | ✅ Yes | SecurityContext | Google managed |
| AKS | ✅ Yes | SecurityContext | Azure managed |

### Example Configurations

#### OpenShift
```yaml
# Custom Security Context Constraint for OpenShift
apiVersion: security.openshift.io/v1
kind: SecurityContextConstraints
metadata:
  name: safer-ring-scc
allowPrivilegedContainer: false
allowedCapabilities:
- SYS_ADMIN
runAsUser:
  type: MustRunAsRange
  uidRangeMin: 1000
  uidRangeMax: 65535
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: safer-ring-sa
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: safer-ring-scc-binding
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: system:openshift:scc:safer-ring-scc
subjects:
- kind: ServiceAccount
  name: safer-ring-sa
  namespace: default
```

## Development Environment Setup

### Local Development

#### Linux
```bash
# Check io_uring support
uname -r
cat /proc/version

# Install development dependencies
sudo apt-get update
sudo apt-get install build-essential pkg-config liburing-dev

# Build and test
cargo build
cargo test
```

#### macOS (Development/Testing)
```bash
# Install Rust and dependencies
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install llvm

# Note: io_uring not available, will use stub backend
cargo build
cargo test  # Tests will use stub backend
```

#### Windows (Development/Testing)
```powershell
# Install Rust
winget install Rustlang.Rustup

# Note: io_uring not available, will use stub backend
cargo build
cargo test  # Tests will use stub backend
```

### CI/CD Pipeline Compatibility

#### GitHub Actions
```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        include:
        - os: ubuntu-latest
          backend_expected: "IoUring or Epoll"
        - os: macos-latest
          backend_expected: "Stub"
        - os: windows-latest
          backend_expected: "Stub"
    
    runs-on: ${{ matrix.os }}
    steps:
    - uses: actions/checkout@v3
    - uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        
    # Linux: Install io_uring development headers
    - name: Install dependencies (Linux)
      if: matrix.os == 'ubuntu-latest'
      run: sudo apt-get install -y liburing-dev
        
    - name: Build
      run: cargo build --verbose
      
    - name: Test
      run: cargo test --verbose
      
    - name: Test backend detection
      run: cargo run --example backend-info
```

#### GitLab CI
```yaml
stages:
  - test
  - build

test:linux:
  stage: test
  image: ubuntu:22.04
  before_script:
    - apt-get update -qq && apt-get install -y -qq git curl build-essential pkg-config liburing-dev
    - curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    - source ~/.cargo/env
  script:
    - cargo test --verbose
  tags:
    - linux

test:compatibility:
  stage: test
  image: alpine:latest
  before_script:
    - apk add --no-cache rust cargo
  script:
    # Alpine: No io_uring, should use stub backend
    - cargo test --verbose
  tags:
    - alpine
```

## Performance Characteristics by Platform

### Expected Performance Multipliers

| Platform | Backend | Small I/O | Large I/O | High Concurrency | Notes |
|----------|---------|-----------|-----------|-------------------|-------|
| Linux 6.0+ | io_uring | 2.5x | 4.5x | 9x | Best case |
| Linux 5.15+ | io_uring | 2.2x | 3.8x | 7x | Excellent |
| Linux 5.4+ | io_uring | 1.8x | 2.8x | 4.5x | Good |
| Linux any | epoll | 1.0x | 1.0x | 1.0x | Baseline |
| macOS/Win | stub | 0.1x | 0.1x | 0.1x | Testing only |

### Platform-Specific Optimizations

#### Linux with io_uring
```rust
use safer_ring::{SaferRingConfig, Ring};

// High-performance configuration for Linux
let config = SaferRingConfig::high_throughput()
    .with_entries(512)  // Large ring for high concurrency
    .with_kernel_poll(true)  // Kernel polling on supported versions
    .with_cooperative_task_running(true);  // Linux 6.0+ feature

let ring = Ring::with_config(config)?;
```

#### Linux with epoll fallback
```rust
// Optimized for epoll backend
let config = SaferRingConfig::epoll_optimized()
    .with_entries(64)   // Smaller ring sufficient
    .with_batching(true);  // Batch submissions

let ring = Ring::with_config(config)?;
```

#### Non-Linux platforms
```rust
// Minimal overhead for stub backend
let config = SaferRingConfig::stub_optimized()
    .with_entries(8)    // Minimal ring
    .with_minimal_features();

let ring = Ring::with_config(config)?;
```

## Troubleshooting Platform Issues

### Common Problems and Solutions

#### "Operation not permitted" on io_uring_setup
```bash
# Check kernel config
zgrep CONFIG_IO_URING /proc/config.gz

# Check seccomp restrictions
grep Seccomp /proc/self/status

# Check capabilities
capsh --print | grep sys_admin
```

#### Docker "Permission denied"
```bash
# Add capability
docker run --cap-add=SYS_ADMIN your-app

# OR check if privileged mode needed
docker run --privileged your-app

# Check container engine
docker info | grep -i runtime
```

#### Kubernetes pod security violations
```bash
# Check pod security policy
kubectl get psp

# Check security context
kubectl describe pod your-pod | grep -A 10 "Security Context"

# Check node kernel version
kubectl get nodes -o custom-columns=NAME:.metadata.name,KERNEL:.status.nodeInfo.kernelVersion
```

### Diagnostic Tools

#### Runtime Information
```rust
use safer_ring::{Runtime, get_environment_info, is_io_uring_available};

fn diagnose_platform() {
    println!("=== Platform Diagnostics ===");
    
    // Basic availability
    println!("io_uring available: {}", is_io_uring_available());
    
    // Runtime detection
    match Runtime::auto_detect() {
        Ok(runtime) => {
            println!("Backend: {}", runtime.backend().description());
            println!("Performance multiplier: {}x", runtime.backend().performance_multiplier());
            
            let env = runtime.environment();
            println!("CPU cores: {}", env.cpu_count);
            println!("Cloud environment: {}", env.is_cloud_environment());
            
            if let Some(container) = &env.container_runtime {
                println!("Container runtime: {}", container);
            }
            
            if let Some(kernel) = &env.kernel_version {
                println!("Kernel version: {}", kernel);
            }
            
            println!("Performance guidance:");
            for tip in runtime.performance_guidance() {
                println!("  • {}", tip);
            }
        }
        Err(e) => {
            println!("Runtime detection failed: {}", e);
        }
    }
    
    println!("=== End Diagnostics ===");
}
```

#### CLI Tool
```bash
# Build diagnostic tool
cargo build --example platform-info

# Run diagnostics
./target/debug/examples/platform-info

# Output:
# Platform: Linux
# Kernel: 5.15.0-72-generic
# io_uring: Available
# Backend: IoUring
# Container: docker
# Performance: 3.2x expected improvement
```

## Support Lifecycle

### Kernel Support Timeline
- **Linux 5.1+**: Core support maintained
- **Linux 5.4+**: Recommended minimum, LTS kernel
- **Linux 5.15+**: Current LTS, fully supported
- **Linux 6.x**: Latest features, actively developed

### Platform EOL Schedule
- **Amazon Linux 2**: EOL 2025-06-30 (migrate to AL2022)
- **Ubuntu 18.04**: EOL 2023-05-31 (no io_uring)
- **CentOS 8**: EOL 2021-12-31 (migrate to RHEL/Rocky)
- **Debian 10**: EOL 2024-06-01 (limited io_uring)

### Migration Recommendations
1. **2024**: Migrate from kernels < 5.4
2. **2025**: Full adoption of io_uring-optimized code paths  
3. **2026**: Consider deprecating epoll fallback for new projects

This compatibility matrix is updated regularly. For the latest information, see the [release notes](./CHANGELOG.md) and [GitHub discussions](https://github.com/your-repo/safer-ring/discussions).