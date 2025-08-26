// This test should fail to compile because pooled buffers cannot outlive their pool

use safer_ring::{BufferPool, Ring};

fn main() {
    let ring = Ring::new(10).unwrap();
    let mut buffer = {
        let pool = BufferPool::new(10, 1024);
        pool.get().unwrap() // This buffer should not be able to outlive the pool
    };
    
    // This should not compile - buffer references dropped pool
    let _future = ring.read(0, buffer.as_mut_slice());
}