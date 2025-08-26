// This test should fail to compile because batch operations must respect buffer lifetimes

use safer_ring::{Ring, Batch, Operation, PinnedBuffer};

fn main() {
    let ring = Ring::new(10).unwrap();
    let mut batch = Batch::new();
    
    {
        let mut buffer = PinnedBuffer::with_capacity(1024);
        let operation = Operation::read()
            .fd(0)
            .buffer(buffer.as_mut_slice());
        
        batch.add_operation(operation).unwrap();
    } // buffer dropped while still referenced in batch
    
    // This should not compile - buffer lifetime too short
    let _future = ring.submit_batch(batch);
}