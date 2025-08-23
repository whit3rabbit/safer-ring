// This test should fail to compile because results cannot be extracted from incomplete operations

use safer_ring::operation::Operation;
use std::pin::Pin;

fn main() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());
    
    let operation = Operation::read()
        .fd(0)
        .buffer(pinned);
    
    let submitted = operation.submit_with_id(1).unwrap();
    
    // This should not compile - submitted operations don't have into_result method
    let (_result, _buffer) = submitted.into_result();
}