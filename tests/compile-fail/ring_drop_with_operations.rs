// This test should fail to compile because the ring cannot be dropped with operations in flight

use safer_ring::{Ring, PinnedBuffer};

fn main() {
    let buffer = PinnedBuffer::with_capacity(1024);
    
    {
        let ring = Ring::new(10).unwrap();
        let _future = ring.read(0, buffer.as_mut_slice());
        // Ring should not be droppable here with operation in flight
    } // This should cause a compile error
}