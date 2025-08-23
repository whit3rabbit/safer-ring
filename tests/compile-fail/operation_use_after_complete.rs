// This test should fail to compile because completed operations cannot be reused

use safer_ring::operation::{Operation, Completed};
use std::pin::Pin;

fn main() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());
    
    let operation = Operation::read()
        .fd(0)
        .buffer(pinned);
    
    let submitted = operation.submit_with_id(1).unwrap();
    let completed: Operation<_, _, Completed<_>> = submitted.complete_with_result(Ok(100));
    
    // This should not compile - completed operations don't have submit_with_id method
    let _resubmit = completed.submit_with_id(2);
}