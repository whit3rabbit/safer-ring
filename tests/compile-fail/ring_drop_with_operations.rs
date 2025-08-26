/// Compile-fail test: Ring cannot be dropped while operations are in flight.
///
/// This test verifies that the lifetime system prevents dropping a Ring
/// when there are still operations referencing it. The borrow checker
/// should prevent compilation because the returned future borrows the ring.

use safer_ring::{Ring, PinnedBuffer};

fn main() {
    let mut buffer = PinnedBuffer::with_capacity(1024);
    
    {
        let ring = Ring::new(10).unwrap();
        let _future = ring.read(0, buffer.as_mut_slice());
        // The ring cannot be dropped here because _future borrows it
    } // Borrow checker prevents this scope from ending
}