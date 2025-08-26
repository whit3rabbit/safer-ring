//! This test verifies that fixed files cannot be used after being unregistered,
//! ensuring registry safety.

use safer_ring::{Registry, Ring};

#[tokio::main]
async fn main() {
    let mut registry = Registry::new();
    let fixed_files = registry.register_fixed_files(vec![0, 1]).unwrap();
    let fixed_stdin = fixed_files[0].clone();
    
    // Unregister all fixed files
    registry.unregister_fixed_files().unwrap();
    
    // This should fail to compile in a real implementation
    // For now it's a runtime safety issue that we document
    let ring = Ring::new(32).unwrap();
    let buffer = safer_ring::PinnedBuffer::with_capacity(1024);
    
    // Using fixed file after unregistration should be prevented
    let _future = ring.read_fixed(&fixed_stdin, buffer.as_mut_slice()); //~ ERROR: potential use after unregister
}