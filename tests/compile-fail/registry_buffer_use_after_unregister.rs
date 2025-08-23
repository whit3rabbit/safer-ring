use safer_ring::Registry;
use std::pin::Pin;

fn main() {
    let mut registry = Registry::new();
    let buffer = Pin::new(Box::new([0u8; 1024]));
    let registered_buffer = registry.register_buffer(buffer).unwrap();
    
    // Unregister the buffer
    let _returned_buffer = registry.unregister_buffer(registered_buffer).unwrap();
    
    // Try to use the buffer after unregistration - should not compile
    println!("Buffer size: {}", registered_buffer.size()); //~ ERROR: borrow of moved value
}