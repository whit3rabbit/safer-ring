//! Validation logic for batch operations and dependencies.

use crate::error::{Result, SaferRingError};
use std::collections::{HashMap, HashSet};

/// A utility for validating dependencies in batch operations and detecting cycles.
///
/// The `DependencyValidator` provides methods to analyze dependency graphs for batch
/// operations, ensuring that operations can be executed in the correct order without
/// circular dependencies that would cause deadlocks or infinite loops.
///
/// This validator is essential for batch processing systems where operations may
/// depend on the completion of other operations before they can be executed.
///
/// # Examples
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use safer_ring::ring::batch::validation::DependencyValidator;
///
/// let mut dependencies = HashMap::new();
/// dependencies.insert(1, vec![0]); // Operation 1 depends on operation 0
/// dependencies.insert(2, vec![1]); // Operation 2 depends on operation 1
///
/// // Check if the dependencies form a valid DAG (Directed Acyclic Graph)
/// assert!(!DependencyValidator::has_circular_dependencies(&dependencies));
///
/// // Get operations in dependency order
/// let order = DependencyValidator::dependency_order(&dependencies, 3).unwrap();
/// assert_eq!(order, vec![0, 1, 2]);
/// ```
pub struct DependencyValidator;

impl DependencyValidator {
    /// Checks if the dependency graph contains circular dependencies.
    ///
    /// This method uses depth-first search (DFS) with a recursion stack to detect
    /// cycles in the dependency graph. A circular dependency occurs when an operation
    /// transitively depends on itself, which would prevent proper execution ordering.
    ///
    /// # Arguments
    ///
    /// * `dependencies` - A mapping from operation IDs to their list of dependency IDs.
    ///   Each key represents an operation, and its value is a vector of operations
    ///   that must complete before this operation can begin.
    ///
    /// # Returns
    ///
    /// Returns `true` if circular dependencies are detected, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use safer_ring::ring::batch::validation::DependencyValidator;
    ///
    /// // Valid dependency chain: 0 -> 1 -> 2
    /// let mut valid_deps = HashMap::new();
    /// valid_deps.insert(1, vec![0]);
    /// valid_deps.insert(2, vec![1]);
    /// assert!(!DependencyValidator::has_circular_dependencies(&valid_deps));
    ///
    /// // Circular dependency: 0 -> 1 -> 0
    /// let mut circular_deps = HashMap::new();
    /// circular_deps.insert(0, vec![1]);
    /// circular_deps.insert(1, vec![0]);
    /// assert!(DependencyValidator::has_circular_dependencies(&circular_deps));
    /// ```
    ///
    /// # Performance
    ///
    /// Time complexity: O(V + E) where V is the number of operations and E is
    /// the number of dependencies.
    /// Space complexity: O(V) for the visited and recursion stack sets.
    pub fn has_circular_dependencies(dependencies: &HashMap<usize, Vec<usize>>) -> bool {
        // Implement cycle detection using DFS
        fn has_cycle(
            node: usize,
            graph: &HashMap<usize, Vec<usize>>,
            visited: &mut HashSet<usize>,
            rec_stack: &mut HashSet<usize>,
        ) -> bool {
            visited.insert(node);
            rec_stack.insert(node);

            if let Some(neighbors) = graph.get(&node) {
                for &neighbor in neighbors {
                    if (!visited.contains(&neighbor)
                        && has_cycle(neighbor, graph, visited, rec_stack))
                        || rec_stack.contains(&neighbor)
                    {
                        return true;
                    }
                }
            }

            rec_stack.remove(&node);
            false
        }

        let mut visited = HashSet::new();

        for &node in dependencies.keys() {
            if !visited.contains(&node) {
                let mut rec_stack = HashSet::new();
                if has_cycle(node, dependencies, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }

        false
    }

    /// Determines if adding a new dependency would create a circular dependency.
    ///
    /// This method checks whether adding a dependency from `dependent` to `new_dependency`
    /// would introduce a cycle in the dependency graph. This is useful for validating
    /// new dependencies before they are added to prevent circular dependencies.
    ///
    /// # Arguments
    ///
    /// * `dependencies` - The current dependency graph mapping operation IDs to their dependencies
    /// * `dependent` - The operation ID that would depend on `new_dependency`
    /// * `new_dependency` - The operation ID that `dependent` wants to depend on
    ///
    /// # Returns
    ///
    /// Returns `true` if adding this dependency would create a cycle, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use safer_ring::ring::batch::validation::DependencyValidator;
    ///
    /// let mut deps = HashMap::new();
    /// deps.insert(1, vec![0]); // 1 depends on 0
    ///
    /// // Adding dependency: 2 depends on 1 (valid)
    /// assert!(!DependencyValidator::would_create_cycle(&deps, 2, 1));
    ///
    /// // Adding dependency: 0 depends on 1 (would create cycle: 0 -> 1 -> 0)
    /// assert!(DependencyValidator::would_create_cycle(&deps, 0, 1));
    /// ```
    ///
    /// # Performance
    ///
    /// Time complexity: O(V + E) in the worst case, where V is the number of operations
    /// and E is the number of dependencies.
    /// Space complexity: O(V) for the visited set and traversal stack.
    pub fn would_create_cycle(
        dependencies: &HashMap<usize, Vec<usize>>,
        dependent: usize,
        new_dependency: usize,
    ) -> bool {
        // Simple cycle detection: check if new_dependency transitively depends on dependent
        let mut visited = HashSet::new();
        let mut stack = vec![new_dependency];

        while let Some(current) = stack.pop() {
            if current == dependent {
                return true; // Found a cycle
            }

            if visited.contains(&current) {
                continue; // Already processed this node
            }
            visited.insert(current);

            // Add all dependencies of current to the stack
            if let Some(deps) = dependencies.get(&current) {
                stack.extend(deps.iter().copied());
            }
        }

        false
    }

    /// Returns operations sorted in dependency order using topological sorting.
    ///
    /// This method performs a topological sort on the dependency graph to determine
    /// a valid execution order where all dependencies of an operation are completed
    /// before the operation itself. Operations with no dependencies will appear first
    /// in the resulting order.
    ///
    /// # Arguments
    ///
    /// * `dependencies` - A mapping from operation IDs to their list of dependency IDs
    /// * `operation_count` - The total number of operations (0 to operation_count-1)
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<usize>)` containing operation IDs in dependency order, or
    /// `Err(SaferRingError)` if circular dependencies are detected.
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::Io` with `ErrorKind::InvalidInput` if:
    /// - Circular dependencies are detected in the graph
    /// - The dependency graph cannot be topologically sorted
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use safer_ring::ring::batch::validation::DependencyValidator;
    ///
    /// let mut deps = HashMap::new();
    /// deps.insert(2, vec![0, 1]); // Operation 2 depends on 0 and 1
    /// deps.insert(1, vec![0]);    // Operation 1 depends on 0
    /// // Operation 0 has no dependencies
    ///
    /// let order = DependencyValidator::dependency_order(&deps, 3).unwrap();
    /// // Valid orders: [0, 1, 2] (dependencies come before dependents)
    /// assert_eq!(order.first(), Some(&0)); // 0 should come first
    /// assert!(order.iter().position(|&x| x == 1).unwrap() <
    ///         order.iter().position(|&x| x == 2).unwrap()); // 1 before 2
    /// ```
    ///
    /// # Performance
    ///
    /// Time complexity: O(V + E) where V is the number of operations and E is
    /// the number of dependencies.
    /// Space complexity: O(V) for the visited sets and result vector.
    pub fn dependency_order(
        dependencies: &HashMap<usize, Vec<usize>>,
        operation_count: usize,
    ) -> Result<Vec<usize>> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        // Topological sort using DFS
        for i in 0..operation_count {
            if !visited.contains(&i) {
                Self::visit_node(
                    i,
                    dependencies,
                    &mut visited,
                    &mut temp_visited,
                    &mut result,
                )?;
            }
        }

        // Note: No reverse needed - DFS post-order already gives correct dependency order
        Ok(result)
    }

    /// Recursively visits a node in the dependency graph for topological sorting.
    ///
    /// This is a helper method that implements the recursive depth-first search
    /// component of topological sorting. It visits all dependencies of a node
    /// before adding the node itself to the result, ensuring proper dependency order.
    ///
    /// # Arguments
    ///
    /// * `node` - The operation ID to visit
    /// * `dependencies` - The dependency graph
    /// * `visited` - Set of permanently visited nodes
    /// * `temp_visited` - Set of temporarily visited nodes (for cycle detection)
    /// * `result` - The result vector being built in dependency order
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or `Err(SaferRingError)` if a circular
    /// dependency is detected.
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::Io` with `ErrorKind::InvalidInput` if a circular
    /// dependency is detected (when a node is found in the temporary visited set).
    fn visit_node(
        node: usize,
        dependencies: &HashMap<usize, Vec<usize>>,
        visited: &mut HashSet<usize>,
        temp_visited: &mut HashSet<usize>,
        result: &mut Vec<usize>,
    ) -> Result<()> {
        if temp_visited.contains(&node) {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Circular dependency detected",
            )));
        }

        if visited.contains(&node) {
            return Ok(());
        }

        temp_visited.insert(node);

        // Visit all dependencies first
        if let Some(deps) = dependencies.get(&node) {
            for &dep in deps {
                Self::visit_node(dep, dependencies, visited, temp_visited, result)?;
            }
        }

        temp_visited.remove(&node);
        visited.insert(node);
        result.push(node);

        Ok(())
    }
}
