# Contributing to Safer-Ring

Thank you for your interest in contributing to safer-ring! This guide will help you get started.

## 🎯 Project Goals

- **Memory safety**: Provide compile-time guarantees against use-after-free and data races
- **Zero-cost abstractions**: No runtime overhead for safety guarantees
- **Ergonomic APIs**: Make io_uring accessible to Rust developers
- **Comprehensive safety**: 24+ compile-fail tests verify safety invariants

## 🏗️ Development Setup

### Prerequisites

- Rust 1.70+ (2021 edition)
- Linux 5.1+ for full functionality (development possible on any platform)

### Getting Started

```bash
git clone https://github.com/whit3rabbit/safer-ring.git
cd safer-ring
cargo build
cargo test
```

### Development Commands

```bash
# Build the project
cargo build

# Run all tests (including compile-fail safety tests)
cargo test

# Run only library unit tests
cargo test --lib

# Run linting
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Build documentation
cargo doc --open

# Run examples (works on all platforms)
cargo run --example async_demo
```

## 🧪 Testing Strategy

The project uses multiple levels of testing:

### Unit Tests
```bash
cargo test --lib
```
Test individual components and basic functionality.

### Integration Tests  
```bash
cargo test --test batch_operations
cargo test --test network_operations
```
Test complete workflows and cross-platform behavior.

### Compile-Fail Tests
```bash
cargo test compile_fail_tests
```
**Critical**: These tests verify that unsafe code patterns fail to compile. Any changes to the safety model must ensure these tests continue to fail.

### Platform Testing
- **Linux**: Full io_uring functionality
- **Non-Linux**: Stub implementations for cross-platform development

## 📝 Code Standards

### Rust Style
- Follow `rustfmt` defaults
- Use `cargo clippy` and address all warnings
- Prefer explicit error handling over panics
- Document all public APIs with examples

### Safety Guidelines
- **Never introduce `unsafe` code** without extensive documentation and testing
- **Lifetime safety is paramount**: Buffer lifetimes must be proven at compile time
- **Test safety invariants**: Add compile-fail tests for new safety requirements
- **Document ownership semantics** clearly in API docs

### Architecture Principles
1. **Ownership transfer over borrowing** for buffer safety
2. **Type-state machines** for operation lifecycle
3. **Compile-time verification** over runtime checks
4. **Platform abstraction** for broad compatibility

## 🐛 Bug Reports

When reporting bugs, please include:

- **Platform**: OS and kernel version (especially for Linux/io_uring issues)
- **Reproduction**: Minimal code example
- **Expected vs actual behavior**
- **Rust version** and dependency versions

Use the GitHub issue templates when available.

## ✨ Feature Requests

Before requesting features:
1. Check if it's already implemented (the project is quite comprehensive)
2. Consider if it maintains memory safety guarantees
3. Ensure it aligns with zero-cost abstraction principles

## 🔧 Development Areas

### High-Impact Contributions

1. **Documentation improvements** - Examples, guides, and API docs
2. **Platform compatibility** - Testing on different Linux distributions and kernels
3. **Performance optimizations** - Benchmarking and optimization (while maintaining safety)
4. **Example applications** - Real-world use cases

### Advanced Contributions

1. **Safety model enhancements** - New compile-fail tests or safety patterns
2. **io_uring feature support** - New io_uring operations (with safety guarantees)
3. **Cross-platform backends** - Alternative backends for non-Linux platforms

## 📋 Pull Request Process

1. **Fork** the repository and create a feature branch
2. **Implement** your changes following the code standards
3. **Test thoroughly**:
   ```bash
   cargo test
   cargo clippy --all-targets --all-features
   cargo fmt --check
   ```
4. **Write tests** for new functionality
5. **Update documentation** if needed
6. **Submit PR** with clear description of changes

### PR Requirements

- [ ] All tests pass (including compile-fail tests)
- [ ] No clippy warnings
- [ ] Code is formatted with rustfmt
- [ ] New features have tests
- [ ] Documentation updated if needed
- [ ] Safety guarantees preserved

## 🏛️ Code Review Guidelines

### For Reviewers
- **Safety first**: Verify that changes maintain memory safety guarantees
- **Test coverage**: Ensure adequate testing, especially for new safety-critical code
- **Documentation**: Check that public APIs are well-documented
- **Performance**: Consider performance implications of changes

### For Contributors  
- **Be patient**: Safety-critical code requires thorough review
- **Provide context**: Explain the reasoning behind design choices
- **Address feedback**: Take reviewer feedback seriously, especially on safety

## 🎓 Learning Resources

### Understanding io_uring
- [Efficient IO with io_uring](https://kernel.dk/io_uring.pdf)
- [Lord of the io_uring](https://unixism.net/loti/)

### Rust Safety Patterns
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### Project Architecture
- [`CLAUDE.md`](CLAUDE.md) - Development guide and architecture overview
- [`src/lib.rs`](src/lib.rs) - API documentation with examples

## 📞 Questions?

- **GitHub Discussions** for design questions and usage help
- **GitHub Issues** for bugs and feature requests  
- **Code review comments** for implementation details

Thank you for contributing to safer-ring! 🦀✨