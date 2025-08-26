//! This test verifies that OwnedBuffer cannot be used after being moved
//! into an operation, ensuring the ownership transfer model works correctly.

use safer_ring::{Ring, OwnedBuffer};

#[tokio::main]
async fn main() {
    let mut ring = Ring::new(32).unwrap();
    let buffer = OwnedBuffer::new(1024);
    
    // Move buffer into operation
    let _future = ring.read_owned(0, buffer);
    
    // This should fail to compile - buffer has been moved
    println!("Buffer size: {}", buffer.size()); //~ ERROR
}