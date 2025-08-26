//! Advanced io_uring features and optimizations.

pub mod buffer_group;
pub mod config;
pub mod feature_detection;
pub mod multi_shot;

pub use buffer_group::BufferGroup;
pub use config::AdvancedConfig;
pub use feature_detection::{AvailableFeatures, FeatureDetector, KernelVersion};
pub use multi_shot::MultiShotConfig;
