// This test should fail to compile because the buffer doesn't outlive the operation

use safer_ring::{Ring, PinnedBuffer};

fn main() {
    let ring = Ring::new(10).unwrap();
    
    {
        let buffer = PinnedBuffer::with_capacity(1024);
        // This should not compile - buffer lifetime is too short
        let _future = ring.read(0, buffer.as_mut_slice());
    } // buffer dropped while operation is pending
}