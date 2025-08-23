//! Compile-fail test for operation submission lifetime constraints.
//!
//! This test verifies that the type system prevents buffer lifetime violations
//! that could lead to use-after-free bugs.

use safer_ring::{Ring, operation::Operation};
use std::pin::Pin;

fn main() {
    let ring = Ring::new(32).unwrap();
    
    {
        let mut buffer = vec![0u8; 1024];
        let pinned_buffer = Pin::new(buffer.as_mut_slice());
        
        let operation = Operation::read()
            .fd(0)
            .buffer(pinned_buffer);
        
        let _submitted = ring.submit(operation).unwrap();
        //~^ ERROR: `buffer` does not live long enough
    } // buffer is dropped here but operation is still tracked by ring
    
    // Ring continues to exist, but buffer is gone - this should not compile
}