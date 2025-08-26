//! Operation type definitions and utilities.

/// Type of I/O operation to perform.
///
/// This enum represents the different types of operations that can be submitted
/// to io_uring. Each variant corresponds to a specific system call or operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OperationType {
    /// Read data from a file descriptor into a buffer
    Read = 0,
    /// Write data from a buffer to a file descriptor
    Write = 1,
    /// Accept a connection on a listening socket
    Accept = 2,
    /// Send data on a socket
    Send = 3,
    /// Receive data from a socket
    Recv = 4,
    /// Vectored read operation (readv)
    ReadVectored = 5,
    /// Vectored write operation (writev)
    WriteVectored = 6,
}

impl OperationType {
    /// Returns true if this operation type requires a buffer.
    ///
    /// Accept operations don't require a buffer as they only return
    /// a new file descriptor for the accepted connection.
    #[inline]
    pub const fn requires_buffer(self) -> bool {
        match self {
            Self::Read
            | Self::Write
            | Self::Send
            | Self::Recv
            | Self::ReadVectored
            | Self::WriteVectored => true,
            Self::Accept => false,
        }
    }

    /// Returns true if this operation type is a read-like operation.
    ///
    /// Read-like operations populate the buffer with data from the kernel.
    #[inline]
    pub const fn is_read_like(self) -> bool {
        matches!(self, Self::Read | Self::Recv | Self::ReadVectored)
    }

    /// Returns true if this operation type is a write-like operation.
    ///
    /// Write-like operations send data from the buffer to the kernel.
    #[inline]
    pub const fn is_write_like(self) -> bool {
        matches!(self, Self::Write | Self::Send | Self::WriteVectored)
    }

    /// Returns true if this operation type is vectored.
    ///
    /// Vectored operations use multiple buffers (scatter-gather I/O).
    #[inline]
    pub const fn is_vectored(self) -> bool {
        matches!(self, Self::ReadVectored | Self::WriteVectored)
    }
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Accept => write!(f, "accept"),
            Self::Send => write!(f, "send"),
            Self::Recv => write!(f, "recv"),
            Self::ReadVectored => write!(f, "readv"),
            Self::WriteVectored => write!(f, "writev"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type_properties() {
        assert!(OperationType::Read.requires_buffer());
        assert!(OperationType::Write.requires_buffer());
        assert!(!OperationType::Accept.requires_buffer());
        assert!(OperationType::Send.requires_buffer());
        assert!(OperationType::Recv.requires_buffer());
        assert!(OperationType::ReadVectored.requires_buffer());
        assert!(OperationType::WriteVectored.requires_buffer());

        assert!(OperationType::Read.is_read_like());
        assert!(!OperationType::Write.is_read_like());
        assert!(OperationType::Recv.is_read_like());
        assert!(OperationType::ReadVectored.is_read_like());
        assert!(!OperationType::WriteVectored.is_read_like());

        assert!(!OperationType::Read.is_write_like());
        assert!(OperationType::Write.is_write_like());
        assert!(OperationType::Send.is_write_like());
        assert!(!OperationType::ReadVectored.is_write_like());
        assert!(OperationType::WriteVectored.is_write_like());

        assert!(!OperationType::Read.is_vectored());
        assert!(!OperationType::Write.is_vectored());
        assert!(OperationType::ReadVectored.is_vectored());
        assert!(OperationType::WriteVectored.is_vectored());
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(OperationType::Read.to_string(), "read");
        assert_eq!(OperationType::Write.to_string(), "write");
        assert_eq!(OperationType::Accept.to_string(), "accept");
        assert_eq!(OperationType::Send.to_string(), "send");
        assert_eq!(OperationType::Recv.to_string(), "recv");
        assert_eq!(OperationType::ReadVectored.to_string(), "readv");
        assert_eq!(OperationType::WriteVectored.to_string(), "writev");
    }
}
