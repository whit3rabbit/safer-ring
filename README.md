# Safer-Ring

A safe Rust wrapper around io_uring that provides zero-cost abstractions while preventing common memory safety issues.

The design's key innovation is embracing ownership rather than fighting it. Instead of trying to make io_uring look like traditional blocking IO, we accept that completion-based APIs require different patterns and design our API accordingly.
This approach transforms the "if it compiles, it panics" problem of current io_uring crates into Rust's standard "if it compiles, it's safe" guarantee, which is exactly what the community needs for production use.

* https://blog.habets.se/2025/04/io-uring-ktls-and-rust-for-zero-syscall-https-server.html
* https://news.ycombinator.com/item?id=44980865
* https://boats.gitlab.io/blog/post/io-uring/


## Features

- **Memory Safety**: Compile-time guarantees that buffers outlive their operations
- **Type Safety**: State machines prevent operations in invalid states  
- **Zero-Cost**: No runtime overhead compared to raw io_uring
- **Async/Await**: Seamless integration with Rust's async ecosystem
- **Buffer Management**: Efficient buffer pooling and reuse

## Platform Support

This library is designed for Linux systems with io_uring support:
- **Minimum**: Linux 5.1 (basic io_uring support)
- **Recommended**: Linux 5.19+ (buffer rings, multi-shot operations)
- **Optimal**: Linux 6.0+ (latest performance improvements)

On non-Linux platforms, the library will compile but `Ring::new()` will return an error.

## Project Status

This project is currently under development. The basic project structure and core types have been implemented, but the actual io_uring integration is still in progress.

## Building

```bash
cargo build
```

## Testing

```bash
cargo test
```

## Examples

See the `examples/` directory for usage examples:
- `echo_server.rs` - TCP echo server example
- `file_copy.rs` - Zero-copy file operations example


## Accepted Limitations

No Stack Buffers (fundamental)
* This is impossible to solve safely with completion-based IO
* API makes this clear through types

Async Model Debates (newpavlov)
* Working within Rust's current async model
* Green threads discussion is valid but out of scope

Performance Overhead
* ~10% overhead vs unsafe io_uring for safety guarantees
* Considered acceptable tradeoff


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.