// This test should fail to compile because futures must not outlive their buffers

use safer_ring::{Ring, PinnedBuffer, ReadFuture};

fn create_future() -> ReadFuture<'static, 'static> {
    let ring = Ring::new(10).unwrap();
    let mut buffer = PinnedBuffer::with_capacity(1024);
    
    // This should not compile - future cannot have 'static lifetime
    // when buffer and ring have local lifetimes
    ring.read(0, buffer.as_mut_slice()).unwrap()
}

fn main() {
    let _future = create_future();
}