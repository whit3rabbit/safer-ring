// This test should fail to compile because building operations don't have id() method

use safer_ring::operation::Operation;

fn main() {
    let operation = Operation::read()
        .fd(0);
    
    // This should not compile - building operations don't have id() method
    let _id = operation.id();
}