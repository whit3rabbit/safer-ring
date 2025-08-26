//! Advanced configuration for io_uring operations.

/// Advanced features configuration for io_uring operations.
///
/// Enables configuration of advanced io_uring features that may not be
/// available on all kernel versions. Features are disabled by default
/// to ensure compatibility.
#[derive(Debug, Clone, Default)]
pub struct AdvancedConfig {
    /// Enable buffer selection for read operations
    pub buffer_selection: bool,
    /// Enable multi-shot operations where supported
    pub multi_shot: bool,
    /// Enable provided buffers for zero-copy operations
    pub provided_buffers: bool,
    /// Enable fast poll for network operations
    pub fast_poll: bool,
    /// Enable submission queue polling
    pub sq_poll: bool,
    /// Enable io_uring kernel thread affinity
    pub sq_thread_cpu: Option<u32>,
    /// Enable cooperative task running
    pub coop_taskrun: bool,
    /// Enable task work defer
    pub defer_taskrun: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AdvancedConfig::default();

        assert!(!config.buffer_selection);
        assert!(!config.multi_shot);
        assert!(!config.provided_buffers);
        assert!(!config.fast_poll);
        assert!(!config.sq_poll);
        assert!(config.sq_thread_cpu.is_none());
        assert!(!config.coop_taskrun);
        assert!(!config.defer_taskrun);
    }
}
