//! Tests for registry functionality.

use super::*;
use std::pin::Pin;

/// Test basic registry creation and properties
mod basic_functionality {
    use super::*;

    #[test]
    fn new_registry_is_empty() {
        let registry = Registry::new();
        assert_eq!(registry.fd_count(), 0);
        assert_eq!(registry.buffer_count(), 0);
        assert_eq!(registry.fds_in_use_count(), 0);
        assert_eq!(registry.buffers_in_use_count(), 0);
    }

    #[test]
    fn default_registry_is_empty() {
        let registry = Registry::default();
        assert_eq!(registry.fd_count(), 0);
        assert_eq!(registry.buffer_count(), 0);
    }
}

/// Test file descriptor registration
mod fd_registration {
    use super::*;

    #[test]
    fn register_valid_fd() {
        let mut registry = Registry::new();
        let result = registry.register_fd(0);
        assert!(result.is_ok());

        let registered_fd = result.unwrap();
        assert_eq!(registered_fd.index(), 0);
        assert_eq!(registered_fd.raw_fd(), 0);
        assert_eq!(registry.fd_count(), 1);
    }

    #[test]
    fn register_multiple_fds() {
        let mut registry = Registry::new();

        let fd1 = registry.register_fd(0).unwrap();
        let fd2 = registry.register_fd(1).unwrap();
        let fd3 = registry.register_fd(2).unwrap();

        assert_eq!(fd1.index(), 0);
        assert_eq!(fd2.index(), 1);
        assert_eq!(fd3.index(), 2);
        assert_eq!(registry.fd_count(), 3);
    }

    #[test]
    fn register_negative_fd_fails() {
        let mut registry = Registry::new();
        let result = registry.register_fd(-1);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn is_fd_registered_works() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(5).unwrap();

        assert!(registry.is_fd_registered(&registered_fd));
        assert_eq!(registry.fd_count(), 1);
    }

    #[test]
    fn get_raw_fd_works() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(42).unwrap();

        let raw_fd = registry.get_raw_fd(&registered_fd).unwrap();
        assert_eq!(raw_fd, 42);
    }
}

/// Test buffer registration
mod buffer_registration {
    use super::*;

    #[test]
    fn register_valid_buffer() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([0u8; 1024]));
        let size = buffer.len();

        let result = registry.register_buffer(buffer);
        assert!(result.is_ok());

        let registered_buffer = result.unwrap();
        assert_eq!(registered_buffer.index(), 0);
        assert_eq!(registered_buffer.size(), size);
        assert_eq!(registry.buffer_count(), 1);
    }

    #[test]
    fn register_multiple_buffers() {
        let mut registry = Registry::new();

        let buffer1 = Pin::new(Box::new([0u8; 512]));
        let buffer2 = Pin::new(Box::new([0u8; 1024]));
        let buffer3 = Pin::new(Box::new([0u8; 2048]));

        let reg1 = registry.register_buffer(buffer1).unwrap();
        let reg2 = registry.register_buffer(buffer2).unwrap();
        let reg3 = registry.register_buffer(buffer3).unwrap();

        assert_eq!(reg1.index(), 0);
        assert_eq!(reg1.size(), 512);
        assert_eq!(reg2.index(), 1);
        assert_eq!(reg2.size(), 1024);
        assert_eq!(reg3.index(), 2);
        assert_eq!(reg3.size(), 2048);
        assert_eq!(registry.buffer_count(), 3);
    }

    #[test]
    fn register_empty_buffer_fails() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([]));

        let result = registry.register_buffer(buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn is_buffer_registered_works() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([0u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();

        assert!(registry.is_buffer_registered(&registered_buffer));
        assert_eq!(registry.buffer_count(), 1);
    }

    #[test]
    fn get_buffer_works() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([42u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();

        let buffer_ref = registry.get_buffer(&registered_buffer).unwrap();
        assert_eq!(buffer_ref.len(), 1024);
        assert_eq!(buffer_ref[0], 42);
    }
}

/// Test unregistration functionality
mod unregistration {
    use super::*;

    #[test]
    fn unregister_fd_success() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(5).unwrap();

        assert_eq!(registry.fd_count(), 1);
        assert!(registry.is_fd_registered(&registered_fd));

        let result = registry.unregister_fd(registered_fd);
        assert!(result.is_ok());
        assert_eq!(registry.fd_count(), 0);
    }

    #[test]
    fn unregister_buffer_success() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([42u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();

        assert_eq!(registry.buffer_count(), 1);
        assert!(registry.is_buffer_registered(&registered_buffer));

        let result = registry.unregister_buffer(registered_buffer);
        assert!(result.is_ok());

        let returned_buffer = result.unwrap();
        assert_eq!(returned_buffer.len(), 1024);
        assert_eq!(returned_buffer[0], 42);
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn unregister_fd_in_use_fails() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(5).unwrap();

        // Mark as in use
        registry.mark_fd_in_use(&registered_fd).unwrap();
        assert_eq!(registry.fds_in_use_count(), 1);

        // Try to unregister - should fail
        let result = registry.unregister_fd(registered_fd);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::BufferInFlight => {}
            _ => panic!("Expected BufferInFlight error"),
        }

        // Clean up for drop safety - mark as not in use
        // We need to recreate the RegisteredFd since it was consumed
        let registered_fd = RegisteredFd { index: 0, fd: 5 };
        registry.mark_fd_not_in_use(&registered_fd);
    }

    #[test]
    fn unregister_buffer_in_use_fails() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([0u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();

        // Mark as in use
        registry.mark_buffer_in_use(&registered_buffer).unwrap();
        assert_eq!(registry.buffers_in_use_count(), 1);

        // Try to unregister - should fail
        let result = registry.unregister_buffer(registered_buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::BufferInFlight => {}
            _ => panic!("Expected BufferInFlight error"),
        }

        // Clean up for drop safety - mark as not in use
        // We need to recreate the RegisteredBuffer since it was consumed
        let registered_buffer = RegisteredBuffer {
            index: 0,
            size: 1024,
        };
        registry.mark_buffer_not_in_use(&registered_buffer);
    }

    #[test]
    fn unregister_nonexistent_fd_fails() {
        let mut registry = Registry::new();

        // Create a fake registered fd
        let fake_fd = RegisteredFd { index: 999, fd: 5 };

        let result = registry.unregister_fd(fake_fd);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::NotRegistered => {}
            _ => panic!("Expected NotRegistered error"),
        }
    }

    #[test]
    fn unregister_nonexistent_buffer_fails() {
        let mut registry = Registry::new();

        // Create a fake registered buffer
        let fake_buffer = RegisteredBuffer {
            index: 999,
            size: 1024,
        };

        let result = registry.unregister_buffer(fake_buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::NotRegistered => {}
            _ => panic!("Expected NotRegistered error"),
        }
    }
}

/// Test usage tracking
mod usage_tracking {
    use super::*;

    #[test]
    fn mark_fd_in_use_works() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(5).unwrap();

        assert_eq!(registry.fds_in_use_count(), 0);

        registry.mark_fd_in_use(&registered_fd).unwrap();
        assert_eq!(registry.fds_in_use_count(), 1);

        registry.mark_fd_not_in_use(&registered_fd);
        assert_eq!(registry.fds_in_use_count(), 0);
    }

    #[test]
    fn mark_buffer_in_use_works() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([0u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();

        assert_eq!(registry.buffers_in_use_count(), 0);

        registry.mark_buffer_in_use(&registered_buffer).unwrap();
        assert_eq!(registry.buffers_in_use_count(), 1);

        registry.mark_buffer_not_in_use(&registered_buffer);
        assert_eq!(registry.buffers_in_use_count(), 0);
    }

    #[test]
    fn mark_nonexistent_fd_in_use_fails() {
        let mut registry = Registry::new();

        let fake_fd = RegisteredFd { index: 999, fd: 5 };

        let result = registry.mark_fd_in_use(&fake_fd);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::NotRegistered => {}
            _ => panic!("Expected NotRegistered error"),
        }
    }

    #[test]
    fn mark_nonexistent_buffer_in_use_fails() {
        let mut registry = Registry::new();

        let fake_buffer = RegisteredBuffer {
            index: 999,
            size: 1024,
        };

        let result = registry.mark_buffer_in_use(&fake_buffer);
        assert!(result.is_err());

        match result.unwrap_err() {
            SaferRingError::NotRegistered => {}
            _ => panic!("Expected NotRegistered error"),
        }
    }
}

/// Test slot reuse after unregistration
mod slot_reuse {
    use super::*;

    #[test]
    fn fd_slot_reuse_works() {
        let mut registry = Registry::new();

        // Register and unregister an fd
        let fd1 = registry.register_fd(5).unwrap();
        assert_eq!(fd1.index(), 0);
        registry.unregister_fd(fd1).unwrap();

        // Register another fd - should reuse the slot
        let fd2 = registry.register_fd(10).unwrap();
        assert_eq!(fd2.index(), 0);
        assert_eq!(fd2.raw_fd(), 10);
    }

    #[test]
    fn buffer_slot_reuse_works() {
        let mut registry = Registry::new();

        // Register and unregister a buffer
        let buffer1 = Pin::new(Box::new([1u8; 512]));
        let reg1 = registry.register_buffer(buffer1).unwrap();
        assert_eq!(reg1.index(), 0);
        let _returned = registry.unregister_buffer(reg1).unwrap();

        // Register another buffer - should reuse the slot
        let buffer2 = Pin::new(Box::new([2u8; 1024]));
        let reg2 = registry.register_buffer(buffer2).unwrap();
        assert_eq!(reg2.index(), 0);
        assert_eq!(reg2.size(), 1024);
    }

    #[test]
    fn mixed_registration_and_slot_reuse() {
        let mut registry = Registry::new();

        // Register multiple resources
        let fd1 = registry.register_fd(1).unwrap();
        let fd2 = registry.register_fd(2).unwrap();
        let buffer1 = Pin::new(Box::new([0u8; 512]));
        let reg1 = registry.register_buffer(buffer1).unwrap();

        assert_eq!(fd1.index(), 0);
        assert_eq!(fd2.index(), 1);
        assert_eq!(reg1.index(), 0);

        // Unregister the first fd
        registry.unregister_fd(fd1).unwrap();

        // Register a new fd - should reuse slot 0
        let fd3 = registry.register_fd(3).unwrap();
        assert_eq!(fd3.index(), 0);
        assert_eq!(fd3.raw_fd(), 3);

        // Other resources should be unaffected
        assert!(registry.is_fd_registered(&fd2));
        assert!(registry.is_buffer_registered(&reg1));
    }
}

/// Test registry lifecycle and safety
mod lifecycle_safety {
    use super::*;

    #[test]
    #[should_panic(expected = "Registry dropped with resources in use")]
    fn drop_with_fds_in_use_panics() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(5).unwrap();
        registry.mark_fd_in_use(&registered_fd).unwrap();

        // Registry should panic on drop because fd is in use
        drop(registry);
    }

    #[test]
    #[should_panic(expected = "Registry dropped with resources in use")]
    fn drop_with_buffers_in_use_panics() {
        let mut registry = Registry::new();
        let buffer = Pin::new(Box::new([0u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();
        registry.mark_buffer_in_use(&registered_buffer).unwrap();

        // Registry should panic on drop because buffer is in use
        drop(registry);
    }

    #[test]
    fn drop_with_no_resources_in_use_succeeds() {
        let mut registry = Registry::new();
        let registered_fd = registry.register_fd(5).unwrap();
        let buffer = Pin::new(Box::new([0u8; 1024]));
        let registered_buffer = registry.register_buffer(buffer).unwrap();

        // Mark as in use then mark as not in use
        registry.mark_fd_in_use(&registered_fd).unwrap();
        registry.mark_buffer_in_use(&registered_buffer).unwrap();
        registry.mark_fd_not_in_use(&registered_fd);
        registry.mark_buffer_not_in_use(&registered_buffer);

        // Should drop successfully
        drop(registry);
    }
}

/// Test compile-time safety properties
mod compile_time_safety {
    // Unused imports removed

    // RegisteredFd and RegisteredBuffer are now Send/Sync for better ergonomics
    // The safety is maintained through the registry's lifetime management
}
