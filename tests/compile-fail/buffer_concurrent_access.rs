// This test should fail to compile because the same buffer cannot be used for multiple operations

use safer_ring::{Ring, PinnedBuffer};

fn main() {
    let ring = Ring::new(10).unwrap();
    let buffer = PinnedBuffer::with_capacity(1024);
    
    // First operation using the buffer
    let _future1 = ring.read(0, buffer.as_mut_slice());
    
    // This should not compile - buffer is already borrowed mutably
    let _future2 = ring.write(1, buffer.as_mut_slice());
}