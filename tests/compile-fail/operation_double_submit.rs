// This test should fail to compile because operations cannot be submitted twice

use safer_ring::operation::{Operation, Building, Submitted};
use std::pin::Pin;

fn main() {
    let mut buffer = vec![0u8; 1024];
    let pinned = Pin::new(buffer.as_mut_slice());
    
    let operation = Operation::read()
        .fd(0)
        .buffer(pinned);
    
    // First submission should work
    let submitted = operation.submit_with_id(1).unwrap();
    
    // This should not compile - submitted operations don't have submit_with_id method
    let _second_submit = submitted.submit_with_id(2);
}