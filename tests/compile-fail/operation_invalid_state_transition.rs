// This test should fail to compile because state transitions must follow the correct order

use safer_ring::{Operation};

fn main() {
    let operation = Operation::read()
        .fd(0);
    
    // This should not compile - cannot directly create a completed operation from building
    let _completed: Operation<'_, '_, safer_ring::operation::Completed<i32>> = operation;
}