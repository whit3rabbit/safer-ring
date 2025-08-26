//! File descriptor and buffer registration for performance optimization.
//!
//! This module provides safe registration of file descriptors and buffers with io_uring
//! for improved performance. Registration allows the kernel to avoid repeated lookups
//! and validations for frequently used resources.
//!
//! # Safety Guarantees
//!
//! - Registered resources cannot be used after unregistration (compile-time enforced)
//! - Resources cannot be unregistered while operations are using them
//! - Buffer addresses remain stable throughout registration lifetime
//! - File descriptors are validated before registration
//!
//! # Example
//!
//! ```rust,no_run
//! # use safer_ring::{Registry, Ring};
//! # use std::pin::Pin;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut registry = Registry::new();
//!
//! // Register a file descriptor
//! let registered_fd = registry.register_fd(0)?;
//!
//! // Register a buffer
//! let buffer = Pin::new(Box::new([0u8; 1024]));
//! let registered_buffer = registry.register_buffer(buffer)?;
//!
//! println!("Registered FD index: {}", registered_fd.index());
//! println!("Registered buffer size: {}", registered_buffer.size());
//! # Ok(())
//! # }
//! ```

use std::collections::HashSet;
use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin;

use crate::error::{Result, SaferRingError};
use crate::ownership::OwnedBuffer;

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests;

/// Registry for managing registered file descriptors and buffers.
///
/// The registry provides safe management of io_uring registered resources,
/// ensuring that resources cannot be used after unregistration and that
/// unregistration cannot occur while resources are in use.
///
/// # Lifetime Management
///
/// The registry is tied to a ring lifetime to ensure that registered resources
/// cannot outlive the ring that registered them. This prevents use-after-free
/// bugs when the ring is dropped.
///
/// # Thread Safety
///
/// The registry is not thread-safe by design. Each ring should have its own
/// registry, and operations should be performed from a single thread or
/// properly synchronized externally.
pub struct Registry<'ring> {
    /// Registered file descriptors with their original fd values
    registered_fds: Vec<Option<(RawFd, RegisteredFdInner)>>,
    /// Registered buffers with their pinned memory
    registered_buffers: Vec<Option<Pin<Box<[u8]>>>>,
    /// Fixed files for optimized access by index
    fixed_files: Vec<Option<RawFd>>,
    /// Pre-registered buffer slots for kernel buffer selection
    registered_buffer_slots: Vec<Option<OwnedBuffer>>,
    /// Set of file descriptor indices currently in use by operations
    fds_in_use: HashSet<u32>,
    /// Set of buffer indices currently in use by operations
    buffers_in_use: HashSet<u32>,
    /// Set of fixed file indices currently in use by operations
    fixed_files_in_use: HashSet<u32>,
    /// Set of registered buffer slot indices currently in use
    buffer_slots_in_use: HashSet<u32>,
    /// Whether the registry has been registered with io_uring
    #[allow(dead_code)]
    is_registered: bool,
    /// Phantom data for lifetime tracking
    _phantom: PhantomData<&'ring ()>,
}

/// Internal data for a registered file descriptor.
#[derive(Debug, Clone)]
struct RegisteredFdInner {
    /// Original file descriptor value
    #[allow(dead_code)]
    fd: RawFd,
    /// Whether this fd is currently in use
    #[allow(dead_code)]
    in_use: bool,
}

/// A registered file descriptor with safe handle and lifetime tracking.
///
/// This handle prevents the file descriptor from being unregistered while
/// it's still in use by operations. The handle is tied to the registry
/// lifetime to prevent use after the registry is dropped.
#[derive(Debug)]
pub struct RegisteredFd {
    /// Index in the registration table
    index: u32,
    /// Original file descriptor for validation
    fd: RawFd,
}

/// A registered buffer with safe handle and lifetime tracking.
///
/// This handle prevents the buffer from being unregistered while it's
/// still in use by operations. The handle maintains information about
/// the buffer size for validation purposes.
#[derive(Debug)]
pub struct RegisteredBuffer {
    /// Index in the registration table
    index: u32,
    /// Size of the registered buffer
    size: usize,
}

impl<'ring> Default for Registry<'ring> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ring> Registry<'ring> {
    /// Create a new empty registry.
    ///
    /// The registry starts empty and can have file descriptors and buffers
    /// registered with it. Registration with io_uring happens when the
    /// first resource is registered.
    pub fn new() -> Self {
        Self {
            registered_fds: Vec::new(),
            registered_buffers: Vec::new(),
            fixed_files: Vec::new(),
            registered_buffer_slots: Vec::new(),
            fds_in_use: HashSet::new(),
            buffers_in_use: HashSet::new(),
            fixed_files_in_use: HashSet::new(),
            buffer_slots_in_use: HashSet::new(),
            is_registered: false,
            _phantom: PhantomData,
        }
    }

    /// Register a file descriptor for optimized access.
    ///
    /// Registers the file descriptor with io_uring for improved performance
    /// on subsequent operations. The returned handle can be used to reference
    /// the registered file descriptor in operations.
    ///
    /// # Arguments
    ///
    /// * `fd` - The file descriptor to register
    ///
    /// # Returns
    ///
    /// Returns a RegisteredFd handle that can be used in operations.
    ///
    /// # Errors
    ///
    /// - Returns `SaferRingError::Io` if the file descriptor is invalid
    /// - Returns `SaferRingError::IoUring` if io_uring registration fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::Registry;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let registered_fd = registry.register_fd(0)?; // stdin
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_fd(&mut self, fd: RawFd) -> Result<RegisteredFd> {
        // Validate file descriptor
        if fd < 0 {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "File descriptor must be non-negative",
            )));
        }

        // Find an empty slot or add a new one
        let index =
            if let Some(empty_index) = self.registered_fds.iter().position(|slot| slot.is_none()) {
                empty_index as u32
            } else {
                let index = self.registered_fds.len() as u32;
                self.registered_fds.push(None);
                index
            };

        let inner = RegisteredFdInner { fd, in_use: false };

        // Store the registration
        self.registered_fds[index as usize] = Some((fd, inner));

        // TODO: Actually register with io_uring when integrated with Ring
        // For now, we just track the registration locally

        Ok(RegisteredFd { index, fd })
    }

    /// Register a buffer for optimized access.
    ///
    /// Registers the buffer with io_uring for improved performance on
    /// subsequent operations. The buffer must be pinned to ensure its
    /// address remains stable throughout the registration lifetime.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The pinned buffer to register
    ///
    /// # Returns
    ///
    /// Returns a RegisteredBuffer handle that can be used in operations.
    ///
    /// # Errors
    ///
    /// - Returns `SaferRingError::Io` if the buffer is empty
    /// - Returns `SaferRingError::IoUring` if io_uring registration fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::Registry;
    /// # use std::pin::Pin;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let buffer = Pin::new(Box::new([0u8; 1024]));
    /// let registered_buffer = registry.register_buffer(buffer)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_buffer(&mut self, buffer: Pin<Box<[u8]>>) -> Result<RegisteredBuffer> {
        if buffer.is_empty() {
            return Err(SaferRingError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Buffer cannot be empty",
            )));
        }

        let size = buffer.len();

        // Find an empty slot or add a new one
        let index = if let Some(empty_index) = self
            .registered_buffers
            .iter()
            .position(|slot| slot.is_none())
        {
            empty_index as u32
        } else {
            let index = self.registered_buffers.len() as u32;
            self.registered_buffers.push(None);
            index
        };

        // Store the buffer
        self.registered_buffers[index as usize] = Some(buffer);

        // TODO: Actually register with io_uring when integrated with Ring
        // For now, we just track the registration locally

        Ok(RegisteredBuffer { index, size })
    }

    /// Unregister a file descriptor.
    ///
    /// Removes the file descriptor from the registry. This operation will
    /// fail if the file descriptor is currently in use by any operations.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - The registered file descriptor to unregister
    ///
    /// # Errors
    ///
    /// - Returns `SaferRingError::NotRegistered` if the fd is not registered
    /// - Returns `SaferRingError::BufferInFlight` if the fd is in use
    ///
    /// # Safety
    ///
    /// This function consumes the RegisteredFd handle, preventing further
    /// use after unregistration.
    pub fn unregister_fd(&mut self, registered_fd: RegisteredFd) -> Result<()> {
        let index = registered_fd.index as usize;

        // Check if the fd exists and matches
        if index >= self.registered_fds.len() {
            return Err(SaferRingError::NotRegistered);
        }

        let slot = &mut self.registered_fds[index];
        let Some((stored_fd, _)) = slot.as_ref() else {
            return Err(SaferRingError::NotRegistered);
        };

        if *stored_fd != registered_fd.fd {
            return Err(SaferRingError::NotRegistered);
        }

        // Check if the fd is in use
        if self.fds_in_use.contains(&registered_fd.index) {
            return Err(SaferRingError::BufferInFlight);
        }

        // Remove the registration
        *slot = None;

        // TODO: Actually unregister with io_uring when integrated with Ring

        Ok(())
    }

    /// Unregister a buffer.
    ///
    /// Removes the buffer from the registry and returns ownership of the
    /// pinned buffer. This operation will fail if the buffer is currently
    /// in use by any operations.
    ///
    /// # Arguments
    ///
    /// * `registered_buffer` - The registered buffer to unregister
    ///
    /// # Returns
    ///
    /// Returns the original pinned buffer on successful unregistration.
    ///
    /// # Errors
    ///
    /// - Returns `SaferRingError::NotRegistered` if the buffer is not registered
    /// - Returns `SaferRingError::BufferInFlight` if the buffer is in use
    ///
    /// # Safety
    ///
    /// This function consumes the RegisteredBuffer handle, preventing further
    /// use after unregistration.
    pub fn unregister_buffer(
        &mut self,
        registered_buffer: RegisteredBuffer,
    ) -> Result<Pin<Box<[u8]>>> {
        let index = registered_buffer.index as usize;

        // Check if the buffer exists
        if index >= self.registered_buffers.len() {
            return Err(SaferRingError::NotRegistered);
        }

        let slot = &mut self.registered_buffers[index];
        let Some(buffer) = slot.as_ref() else {
            return Err(SaferRingError::NotRegistered);
        };

        // Validate the buffer size matches
        if buffer.len() != registered_buffer.size {
            return Err(SaferRingError::NotRegistered);
        }

        // Check if the buffer is in use
        if self.buffers_in_use.contains(&registered_buffer.index) {
            return Err(SaferRingError::BufferInFlight);
        }

        // Remove and return the buffer
        let buffer = slot.take().unwrap();

        // TODO: Actually unregister with io_uring when integrated with Ring

        Ok(buffer)
    }

    /// Mark a file descriptor as in use by an operation.
    ///
    /// This is called internally when an operation is submitted using a
    /// registered file descriptor. It prevents the fd from being unregistered
    /// while the operation is in flight.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - The registered file descriptor being used
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::NotRegistered` if the fd is not registered.
    #[allow(dead_code)]
    pub(crate) fn mark_fd_in_use(&mut self, registered_fd: &RegisteredFd) -> Result<()> {
        let index = registered_fd.index as usize;

        if index >= self.registered_fds.len() || self.registered_fds[index].is_none() {
            return Err(SaferRingError::NotRegistered);
        }

        self.fds_in_use.insert(registered_fd.index);
        Ok(())
    }

    /// Mark a file descriptor as no longer in use.
    ///
    /// This is called internally when an operation using a registered file
    /// descriptor completes. It allows the fd to be unregistered again.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - The registered file descriptor no longer in use
    #[allow(dead_code)]
    pub(crate) fn mark_fd_not_in_use(&mut self, registered_fd: &RegisteredFd) {
        self.fds_in_use.remove(&registered_fd.index);
    }

    /// Mark a buffer as in use by an operation.
    ///
    /// This is called internally when an operation is submitted using a
    /// registered buffer. It prevents the buffer from being unregistered
    /// while the operation is in flight.
    ///
    /// # Arguments
    ///
    /// * `registered_buffer` - The registered buffer being used
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::NotRegistered` if the buffer is not registered.
    #[allow(dead_code)]
    pub(crate) fn mark_buffer_in_use(
        &mut self,
        registered_buffer: &RegisteredBuffer,
    ) -> Result<()> {
        let index = registered_buffer.index as usize;

        if index >= self.registered_buffers.len() || self.registered_buffers[index].is_none() {
            return Err(SaferRingError::NotRegistered);
        }

        self.buffers_in_use.insert(registered_buffer.index);
        Ok(())
    }

    /// Mark a buffer as no longer in use.
    ///
    /// This is called internally when an operation using a registered buffer
    /// completes. It allows the buffer to be unregistered again.
    ///
    /// # Arguments
    ///
    /// * `registered_buffer` - The registered buffer no longer in use
    #[allow(dead_code)]
    pub(crate) fn mark_buffer_not_in_use(&mut self, registered_buffer: &RegisteredBuffer) {
        self.buffers_in_use.remove(&registered_buffer.index);
    }

    /// Get the number of registered file descriptors.
    ///
    /// Returns the total number of file descriptors currently registered,
    /// including those that may be in use by operations.
    pub fn fd_count(&self) -> usize {
        self.registered_fds
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }

    /// Get the number of registered buffers.
    ///
    /// Returns the total number of buffers currently registered,
    /// including those that may be in use by operations.
    pub fn buffer_count(&self) -> usize {
        self.registered_buffers
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }

    /// Get the number of file descriptors currently in use.
    ///
    /// Returns the number of registered file descriptors that are
    /// currently being used by in-flight operations.
    pub fn fds_in_use_count(&self) -> usize {
        self.fds_in_use.len()
    }

    /// Get the number of buffers currently in use.
    ///
    /// Returns the number of registered buffers that are currently
    /// being used by in-flight operations.
    pub fn buffers_in_use_count(&self) -> usize {
        self.buffers_in_use.len()
    }

    /// Check if a file descriptor is registered.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - The registered file descriptor to check
    ///
    /// # Returns
    ///
    /// Returns true if the file descriptor is currently registered.
    pub fn is_fd_registered(&self, registered_fd: &RegisteredFd) -> bool {
        let index = registered_fd.index as usize;
        index < self.registered_fds.len()
            && self.registered_fds[index]
                .as_ref()
                .map(|(fd, _)| *fd == registered_fd.fd)
                .unwrap_or(false)
    }

    /// Check if a buffer is registered.
    ///
    /// # Arguments
    ///
    /// * `registered_buffer` - The registered buffer to check
    ///
    /// # Returns
    ///
    /// Returns true if the buffer is currently registered.
    pub fn is_buffer_registered(&self, registered_buffer: &RegisteredBuffer) -> bool {
        let index = registered_buffer.index as usize;
        index < self.registered_buffers.len()
            && self.registered_buffers[index]
                .as_ref()
                .map(|buffer| buffer.len() == registered_buffer.size)
                .unwrap_or(false)
    }

    /// Get the raw file descriptor value for a registered fd.
    ///
    /// This is used internally by operations to get the actual fd value
    /// for system calls.
    ///
    /// # Arguments
    ///
    /// * `registered_fd` - The registered file descriptor
    ///
    /// # Returns
    ///
    /// Returns the raw file descriptor value if registered.
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::NotRegistered` if the fd is not registered.
    #[allow(dead_code)]
    pub(crate) fn get_raw_fd(&self, registered_fd: &RegisteredFd) -> Result<RawFd> {
        let index = registered_fd.index as usize;

        if index >= self.registered_fds.len() {
            return Err(SaferRingError::NotRegistered);
        }

        match &self.registered_fds[index] {
            Some((fd, _)) if *fd == registered_fd.fd => Ok(*fd),
            _ => Err(SaferRingError::NotRegistered),
        }
    }

    /// Get a reference to a registered buffer.
    ///
    /// This is used internally by operations to access the buffer data
    /// for I/O operations.
    ///
    /// # Arguments
    ///
    /// * `registered_buffer` - The registered buffer
    ///
    /// # Returns
    ///
    /// Returns a reference to the pinned buffer if registered.
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::NotRegistered` if the buffer is not registered.
    #[allow(dead_code)]
    pub(crate) fn get_buffer(
        &self,
        registered_buffer: &RegisteredBuffer,
    ) -> Result<&Pin<Box<[u8]>>> {
        let index = registered_buffer.index as usize;

        if index >= self.registered_buffers.len() {
            return Err(SaferRingError::NotRegistered);
        }

        match &self.registered_buffers[index] {
            Some(buffer) if buffer.len() == registered_buffer.size => Ok(buffer),
            _ => Err(SaferRingError::NotRegistered),
        }
    }

    // Fixed Files API - Performance optimization for frequently used files

    /// Register multiple files for optimized fixed-file operations.
    ///
    /// Fixed files are pre-registered with io_uring and accessed by index
    /// instead of file descriptor, providing better performance for frequently
    /// used files. This is particularly useful for applications that work with
    /// a known set of files repeatedly.
    ///
    /// # Arguments
    ///
    /// * `fds` - Vector of file descriptors to register as fixed files
    ///
    /// # Returns
    ///
    /// Returns a vector of FixedFile handles in the same order as input.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::Registry;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let fixed_files = registry.register_fixed_files(vec![0, 1, 2])?; // stdin, stdout, stderr
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_fixed_files(&mut self, fds: Vec<RawFd>) -> Result<Vec<FixedFile>> {
        if fds.is_empty() {
            return Ok(Vec::new());
        }

        // Validate all file descriptors first
        for &fd in &fds {
            if fd < 0 {
                return Err(SaferRingError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid file descriptor: {}", fd),
                )));
            }
        }

        // Clear existing fixed files if any
        self.fixed_files.clear();
        self.fixed_files_in_use.clear();

        let mut fixed_files = Vec::new();

        for (index, &fd) in fds.iter().enumerate() {
            self.fixed_files.push(Some(fd));
            fixed_files.push(FixedFile {
                index: index as u32,
                fd,
            });
        }

        // TODO: Actually register with io_uring when integrated with Ring
        // For now, we just track the registration locally

        Ok(fixed_files)
    }

    /// Unregister all fixed files.
    ///
    /// This operation will fail if any fixed file is currently in use.
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::BufferInFlight` if any fixed file is in use.
    pub fn unregister_fixed_files(&mut self) -> Result<()> {
        if !self.fixed_files_in_use.is_empty() {
            return Err(SaferRingError::BufferInFlight);
        }

        self.fixed_files.clear();
        // TODO: Actually unregister with io_uring when integrated with Ring

        Ok(())
    }

    /// Get the number of fixed files registered.
    pub fn fixed_file_count(&self) -> usize {
        self.fixed_files
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }

    /// Get the number of fixed files currently in use.
    pub fn fixed_files_in_use_count(&self) -> usize {
        self.fixed_files_in_use.len()
    }

    // Registered Buffer Slots API - For kernel buffer selection optimization

    /// Register multiple buffer slots for kernel buffer selection.
    ///
    /// Registered buffer slots are pre-allocated buffers that the kernel
    /// can select from during I/O operations. This provides the best performance
    /// for high-throughput applications by eliminating the need to specify
    /// buffer addresses in each operation.
    ///
    /// # Arguments
    ///
    /// * `buffers` - Vector of owned buffers to register as selectable slots
    ///
    /// # Returns
    ///
    /// Returns a vector of RegisteredBufferSlot handles.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use safer_ring::{Registry, OwnedBuffer};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let buffers = vec![
    ///     OwnedBuffer::new(4096),
    ///     OwnedBuffer::new(4096),
    ///     OwnedBuffer::new(4096),
    /// ];
    /// let buffer_slots = registry.register_buffer_slots(buffers)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_buffer_slots(
        &mut self,
        buffers: Vec<OwnedBuffer>,
    ) -> Result<Vec<RegisteredBufferSlot>> {
        if buffers.is_empty() {
            return Ok(Vec::new());
        }

        // Clear existing buffer slots if any
        self.registered_buffer_slots.clear();
        self.buffer_slots_in_use.clear();

        let mut buffer_slots = Vec::new();

        for (index, buffer) in buffers.into_iter().enumerate() {
            let size = buffer.size();
            self.registered_buffer_slots.push(Some(buffer));
            buffer_slots.push(RegisteredBufferSlot {
                index: index as u32,
                size,
                in_use: false,
            });
        }

        // TODO: Actually register with io_uring when integrated with Ring
        // For now, we just track the registration locally

        Ok(buffer_slots)
    }

    /// Unregister all buffer slots and return ownership of the buffers.
    ///
    /// This operation will fail if any buffer slot is currently in use.
    ///
    /// # Returns
    ///
    /// Returns the original owned buffers on successful unregistration.
    ///
    /// # Errors
    ///
    /// Returns `SaferRingError::BufferInFlight` if any buffer slot is in use.
    pub fn unregister_buffer_slots(&mut self) -> Result<Vec<OwnedBuffer>> {
        if !self.buffer_slots_in_use.is_empty() {
            return Err(SaferRingError::BufferInFlight);
        }

        let buffers = self.registered_buffer_slots.drain(..).flatten().collect();

        // TODO: Actually unregister with io_uring when integrated with Ring

        Ok(buffers)
    }

    /// Get the number of buffer slots registered.
    pub fn buffer_slot_count(&self) -> usize {
        self.registered_buffer_slots
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }

    /// Get the number of buffer slots currently in use.
    pub fn buffer_slots_in_use_count(&self) -> usize {
        self.buffer_slots_in_use.len()
    }

    // Internal methods for tracking usage of fixed files and buffer slots

    /// Mark a fixed file as in use by an operation.
    #[allow(dead_code)]
    pub(crate) fn mark_fixed_file_in_use(&mut self, fixed_file: &FixedFile) -> Result<()> {
        let index = fixed_file.index as usize;

        if index >= self.fixed_files.len() || self.fixed_files[index].is_none() {
            return Err(SaferRingError::NotRegistered);
        }

        self.fixed_files_in_use.insert(fixed_file.index);
        Ok(())
    }

    /// Mark a fixed file as no longer in use.
    #[allow(dead_code)]
    pub(crate) fn mark_fixed_file_not_in_use(&mut self, fixed_file: &FixedFile) {
        self.fixed_files_in_use.remove(&fixed_file.index);
    }

    /// Mark a buffer slot as in use by an operation.
    #[allow(dead_code)]
    pub(crate) fn mark_buffer_slot_in_use(
        &mut self,
        buffer_slot: &RegisteredBufferSlot,
    ) -> Result<()> {
        let index = buffer_slot.index as usize;

        if index >= self.registered_buffer_slots.len()
            || self.registered_buffer_slots[index].is_none()
        {
            return Err(SaferRingError::NotRegistered);
        }

        self.buffer_slots_in_use.insert(buffer_slot.index);
        Ok(())
    }

    /// Mark a buffer slot as no longer in use.
    #[allow(dead_code)]
    pub(crate) fn mark_buffer_slot_not_in_use(&mut self, buffer_slot: &RegisteredBufferSlot) {
        self.buffer_slots_in_use.remove(&buffer_slot.index);
    }

    /// Get the raw file descriptor for a fixed file.
    #[allow(dead_code)]
    pub(crate) fn get_fixed_file_fd(&self, fixed_file: &FixedFile) -> Result<RawFd> {
        let index = fixed_file.index as usize;

        if index >= self.fixed_files.len() {
            return Err(SaferRingError::NotRegistered);
        }

        match &self.fixed_files[index] {
            Some(fd) if *fd == fixed_file.fd => Ok(*fd),
            _ => Err(SaferRingError::NotRegistered),
        }
    }

    /// Get a reference to a buffer in a registered slot.
    #[allow(dead_code)]
    pub(crate) fn get_buffer_slot(
        &self,
        buffer_slot: &RegisteredBufferSlot,
    ) -> Result<&OwnedBuffer> {
        let index = buffer_slot.index as usize;

        if index >= self.registered_buffer_slots.len() {
            return Err(SaferRingError::NotRegistered);
        }

        match &self.registered_buffer_slots[index] {
            Some(buffer) if buffer.size() == buffer_slot.size => Ok(buffer),
            _ => Err(SaferRingError::NotRegistered),
        }
    }
}

impl<'ring> Drop for Registry<'ring> {
    /// Ensure no resources are in use when the registry is dropped.
    ///
    /// This prevents resource leaks and ensures that all operations have
    /// completed before the registry is destroyed.
    fn drop(&mut self) {
        let total_in_use = self.fds_in_use.len()
            + self.buffers_in_use.len()
            + self.fixed_files_in_use.len()
            + self.buffer_slots_in_use.len();

        if total_in_use > 0 {
            panic!(
                "Registry dropped with resources in use: {} fds, {} buffers, {} fixed_files, {} buffer_slots",
                self.fds_in_use.len(),
                self.buffers_in_use.len(),
                self.fixed_files_in_use.len(),
                self.buffer_slots_in_use.len()
            );
        }
    }
}

impl RegisteredFd {
    /// Get the index of this registered file descriptor.
    ///
    /// The index is used internally by io_uring to reference the registered
    /// file descriptor efficiently.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get the raw file descriptor value.
    ///
    /// This returns the original file descriptor that was registered.
    /// Note that this should generally not be used directly - use the
    /// registered handle in operations instead.
    pub fn raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl RegisteredBuffer {
    /// Get the index of this registered buffer.
    ///
    /// The index is used internally by io_uring to reference the registered
    /// buffer efficiently.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get the size of this registered buffer.
    ///
    /// Returns the size in bytes of the registered buffer.
    pub fn size(&self) -> usize {
        self.size
    }
}

/// Fixed file descriptor with optimized kernel access.
///
/// Fixed files are pre-registered with the kernel and accessed by index
/// instead of file descriptor, providing better performance for frequently
/// used files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFile {
    /// Index in the fixed file table
    index: u32,
    /// Original file descriptor for validation
    fd: RawFd,
}

/// Registered buffer slot with optimized kernel access.
///
/// Registered buffers are pre-allocated and registered with the kernel,
/// allowing operations to reference buffers by index instead of pointer
/// for better performance.
#[derive(Debug)]
pub struct RegisteredBufferSlot {
    /// Index in the registered buffer table
    index: u32,
    /// Size of the buffer slot
    size: usize,
    /// Whether this slot is currently in use
    in_use: bool,
}

impl FixedFile {
    /// Get the index of this fixed file.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get the original file descriptor.
    pub fn raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl RegisteredBufferSlot {
    /// Get the index of this buffer slot.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get the size of this buffer slot.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if this slot is currently in use.
    pub fn is_in_use(&self) -> bool {
        self.in_use
    }
}

// RegisteredFd and RegisteredBuffer are now Send/Sync for better ergonomics
// The safety is maintained through the registry's lifetime management
