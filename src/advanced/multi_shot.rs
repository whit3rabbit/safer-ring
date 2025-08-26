//! Multi-shot operation configuration.

/// Multi-shot operation configuration.
///
/// Multi-shot operations allow a single submission to generate multiple
/// completion events, reducing submission overhead for operations like
/// `accept()` that can be repeated.
#[derive(Debug, Clone, Default)]
pub struct MultiShotConfig {
    /// Maximum number of completions to generate
    pub max_completions: Option<u32>,
    /// Whether to continue on errors
    pub continue_on_error: bool,
    /// Buffer group to use for multi-shot reads
    pub buffer_group_id: Option<u16>,
}

impl MultiShotConfig {
    /// Create a new multi-shot configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use safer_ring::advanced::MultiShotConfig;
    ///
    /// let config = MultiShotConfig::new()
    ///     .with_max_completions(100)
    ///     .with_buffer_group(1);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of completions.
    pub fn with_max_completions(mut self, max: u32) -> Self {
        self.max_completions = Some(max);
        self
    }

    /// Enable continuing on errors.
    pub fn with_continue_on_error(mut self) -> Self {
        self.continue_on_error = true;
        self
    }

    /// Set the buffer group ID for multi-shot reads.
    pub fn with_buffer_group(mut self, group_id: u16) -> Self {
        self.buffer_group_id = Some(group_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MultiShotConfig::default();

        assert!(config.max_completions.is_none());
        assert!(!config.continue_on_error);
        assert!(config.buffer_group_id.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let config = MultiShotConfig::new()
            .with_max_completions(50)
            .with_continue_on_error()
            .with_buffer_group(2);

        assert_eq!(config.max_completions, Some(50));
        assert!(config.continue_on_error);
        assert_eq!(config.buffer_group_id, Some(2));
    }
}
