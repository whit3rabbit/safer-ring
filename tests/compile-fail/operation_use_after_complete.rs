// This test should fail to compile because operations cannot be used after being moved

use safer_ring::operation::Operation;
use std::pin::Pin;

fn main() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());
    
    let operation = Operation::read()
        .fd(0)
        .buffer(pinned);
    
    // Move the operation
    let _moved_operation = operation;
    
    // This should not compile - operation was moved
    let _another_use = operation.fd(1);
}