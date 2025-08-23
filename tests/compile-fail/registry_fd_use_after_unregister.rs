use safer_ring::Registry;

fn main() {
    let mut registry = Registry::new();
    let registered_fd = registry.register_fd(0).unwrap();
    
    // Unregister the fd
    registry.unregister_fd(registered_fd).unwrap();
    
    // Try to use the fd after unregistration - should not compile
    println!("FD index: {}", registered_fd.index()); //~ ERROR: borrow of moved value
}