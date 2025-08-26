//! Buffer group management for provided buffers.

use crate::error::{Result, SaferRingError};
use std::collections::VecDeque;

/// Buffer selection group for managing provided buffers.
///
/// Allows the kernel to select buffers from a pre-registered pool,
/// enabling zero-copy operations and reducing buffer management overhead.
#[derive(Debug)]
pub struct BufferGroup {
    /// Group ID for this buffer set
    pub group_id: u16,
    /// Number of buffers in the group
    pub buffer_count: u32,
    /// Size of each buffer in bytes
    pub buffer_size: u32,
    /// Pre-allocated buffer storage
    #[cfg(target_os = "linux")]
    #[allow(dead_code)] // Will be used when buffer selection is implemented
    buffers: Vec<Box<[u8]>>,
    /// Available buffer indices - using VecDeque for O(1) operations
    available_buffers: VecDeque<u16>,
}

impl BufferGroup {
    /// Create a new buffer group with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `group_id` - Unique identifier for this buffer group
    /// * `buffer_count` - Number of buffers to allocate
    /// * `buffer_size` - Size of each buffer in bytes
    ///
    /// # Errors
    ///
    /// Returns an error if buffer allocation fails or parameters are invalid.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use safer_ring::advanced::BufferGroup;
    ///
    /// let group = BufferGroup::new(1, 100, 4096)?;
    /// # Ok::<(), safer_ring::error::SaferRingError>(())
    /// ```
    pub fn new(group_id: u16, buffer_count: u32, buffer_size: u32) -> Result<Self> {
        if buffer_count == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Buffer count must be greater than 0",
            )));
        }

        if buffer_size == 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Buffer size must be greater than 0",
            )));
        }

        // Pre-allocate all buffers to avoid runtime allocation overhead
        #[cfg(target_os = "linux")]
        let buffers = {
            let mut buffers = Vec::with_capacity(buffer_count as usize);
            for _ in 0..buffer_count {
                // Use boxed slice for stable memory layout
                buffers.push(vec![0u8; buffer_size as usize].into_boxed_slice());
            }
            buffers
        };

        // Initialize all buffer indices as available
        let available_buffers = (0..buffer_count as u16).collect();

        Ok(Self {
            group_id,
            buffer_count,
            buffer_size,
            #[cfg(target_os = "linux")]
            buffers,
            available_buffers,
        })
    }

    /// Get the next available buffer index.
    ///
    /// Returns `None` if no buffers are available.
    #[inline]
    pub fn get_buffer(&mut self) -> Option<u16> {
        self.available_buffers.pop_front()
    }

    /// Return a buffer to the available pool.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer index to return
    ///
    /// # Panics
    ///
    /// Panics in debug mode if `buffer_id` is invalid.
    #[inline]
    pub fn return_buffer(&mut self, buffer_id: u16) {
        debug_assert!(
            buffer_id < self.buffer_count as u16,
            "Invalid buffer ID: {} >= {}",
            buffer_id,
            self.buffer_count
        );

        // Only add valid buffer IDs to prevent corruption
        if buffer_id < self.buffer_count as u16 {
            self.available_buffers.push_back(buffer_id);
        }
    }

    /// Get the number of available buffers.
    #[inline]
    pub fn available_count(&self) -> usize {
        self.available_buffers.len()
    }

    /// Check if any buffers are available.
    #[inline]
    pub fn has_available(&self) -> bool {
        !self.available_buffers.is_empty()
    }

    /// Get the utilization ratio (0.0 to 1.0).
    ///
    /// Returns the fraction of buffers currently in use.
    #[inline]
    pub fn utilization(&self) -> f64 {
        let in_use = self.buffer_count as usize - self.available_buffers.len();
        in_use as f64 / self.buffer_count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_group_creation() {
        let group = BufferGroup::new(1, 10, 4096).unwrap();

        assert_eq!(group.group_id, 1);
        assert_eq!(group.buffer_count, 10);
        assert_eq!(group.buffer_size, 4096);
        assert_eq!(group.available_count(), 10);
        assert!(group.has_available());
        assert_eq!(group.utilization(), 0.0);
    }

    #[test]
    fn test_buffer_allocation() {
        let mut group = BufferGroup::new(1, 5, 1024).unwrap();

        // Get all buffers
        let mut buffer_ids = Vec::new();
        for _ in 0..5 {
            let id = group.get_buffer().unwrap();
            buffer_ids.push(id);
        }

        assert_eq!(group.available_count(), 0);
        assert!(!group.has_available());
        assert_eq!(group.utilization(), 1.0);
        assert!(group.get_buffer().is_none());

        // Return buffers
        for id in buffer_ids {
            group.return_buffer(id);
        }

        assert_eq!(group.available_count(), 5);
        assert!(group.has_available());
        assert_eq!(group.utilization(), 0.0);
    }

    #[test]
    fn test_invalid_parameters() {
        assert!(BufferGroup::new(1, 0, 4096).is_err());
        assert!(BufferGroup::new(1, 10, 0).is_err());
    }
}
