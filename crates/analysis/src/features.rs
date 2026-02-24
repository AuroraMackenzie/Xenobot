use serde::{Deserialize, Serialize};

/// Configuration for feature extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Name of the embedding model to use.
    pub embedding_model: String,
    /// Whether to enable GPU acceleration.
    pub enable_gpu: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            embedding_model: "text-embedding-3-small".to_string(),
            enable_gpu: false,
        }
    }
}

/// Extract numerical features from text for ML processing.
///
/// Converts text characters to numerical representation.
/// TODO: Implement proper embedding extraction with Candle.
pub fn extract_features(text: &str) -> Vec<f32> {
    text.chars().map(|c| c as u32 as f32).take(512).collect()
}
