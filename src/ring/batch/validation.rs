//! Validation logic for batch operations and dependencies.

use crate::error::{Result, SaferRingError};
use std::collections::{HashMap, HashSet};

/// Validates batch dependencies and checks for cycles.
pub struct DependencyValidator;

impl DependencyValidator {
    /// Check if the batch has circular dependencies.
    ///
    /// This method performs a topological sort to detect cycles in the
    /// dependency graph.
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

    /// Check if adding a dependency would create a cycle.
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

    /// Get the operations in dependency order.
    ///
    /// Returns operations sorted such that dependencies come before dependents.
    /// Operations with no dependencies come first.
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

    /// Visit a node in the dependency graph for topological sorting.
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
