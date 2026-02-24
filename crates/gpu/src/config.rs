//! Configuration for GPU acceleration.

use serde::{Deserialize, Serialize};

/// GPU acceleration configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuConfig {
    /// Whether GPU acceleration is enabled.
    pub enabled: bool,

    /// Metal device preference (discrete/integrated/auto).
    pub device_preference: MetalDevicePreference,

    /// Maximum memory to allocate (in MB).
    pub max_memory_mb: u64,

    /// Whether to use MPS (Metal Performance Shaders) for ML.
    pub use_mps: bool,

    /// Whether to use Candle with Metal backend.
    pub use_candle_metal: bool,
}

/// Metal device preference.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MetalDevicePreference {
    /// Let Metal choose the best device.
    Auto,
    /// Prefer discrete GPU (if available).
    Discrete,
    /// Prefer integrated GPU.
    Integrated,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            device_preference: MetalDevicePreference::Auto,
            max_memory_mb: 4096, // 4 GB
            use_mps: true,
            use_candle_metal: true,
        }
    }
}
