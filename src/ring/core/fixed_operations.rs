//! Fixed file operations using registered indices for the Ring.

use super::Ring;
use crate::error::Result;
use crate::future::{ReadFuture, WriteFuture};
use crate::operation::Operation;

impl<'ring> Ring<'ring> {
    /// Read from a fixed file using its registered index.
    ///
    /// Fixed files are pre-registered with the kernel and accessed by index
    /// instead of file descriptor, providing better performance for frequently
    /// used files. The file must be registered with the registry first.
    ///
    /// # Arguments
    ///
    /// * `fixed_file` - The fixed file to read from
    /// * `buffer` - Pinned buffer to read data into
    ///
    /// # Returns
    ///
    /// Returns a ReadFuture that resolves to (bytes_read, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted or if the
    /// fixed file is not registered.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer, Registry};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let fixed_files = registry.register_fixed_files(vec![0])?; // stdin
    /// let fixed_stdin = &fixed_files[0];
    ///
    /// let ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::with_capacity(1024);
    ///
    /// let (bytes_read, buffer) = ring.read_fixed(fixed_stdin, buffer.as_mut_slice()).await?;
    /// println!("Read {} bytes from fixed file", bytes_read);
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_fixed<'buf>(
        &'ring mut self,
        fixed_file: &crate::registry::FixedFile,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<ReadFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::read()
            .fixed_file(fixed_file.clone())
            .buffer(buffer);
        let submitted = self.submit(operation)?;
        Ok(ReadFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    /// Write to a fixed file using its registered index.
    ///
    /// Fixed files are pre-registered with the kernel and accessed by index
    /// instead of file descriptor, providing better performance for frequently
    /// used files. The file must be registered with the registry first.
    ///
    /// # Arguments
    ///
    /// * `fixed_file` - The fixed file to write to
    /// * `buffer` - Pinned buffer containing data to write
    ///
    /// # Returns
    ///
    /// Returns a WriteFuture that resolves to (bytes_written, buffer).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation cannot be submitted or if the
    /// fixed file is not registered.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use safer_ring::{Ring, PinnedBuffer, Registry};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut registry = Registry::new();
    /// let fixed_files = registry.register_fixed_files(vec![1])?; // stdout
    /// let fixed_stdout = &fixed_files[0];
    ///
    /// let ring = Ring::new(32)?;
    /// let mut buffer = PinnedBuffer::from_slice(b"Hello from fixed file!");
    ///
    /// let (bytes_written, buffer) = ring.write_fixed(fixed_stdout, buffer.as_mut_slice()).await?;
    /// println!("Wrote {} bytes to fixed file", bytes_written);
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_fixed<'buf>(
        &'ring mut self,
        fixed_file: &crate::registry::FixedFile,
        buffer: std::pin::Pin<&'buf mut [u8]>,
    ) -> Result<WriteFuture<'ring, 'buf>>
    where
        'buf: 'ring,
    {
        let operation = Operation::write()
            .fixed_file(fixed_file.clone())
            .buffer(buffer);
        let submitted = self.submit(operation)?;
        Ok(WriteFuture::new(
            submitted,
            self,
            self.waker_registry.clone(),
        ))
    }

    // TODO: Add buffer selection operations when future types are implemented
    // The read_with_buffer_selection method would go here once the appropriate
    // future types are implemented for buffer selection operations.
}
