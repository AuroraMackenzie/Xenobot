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

/// Default feature vector size used by local deterministic vectorization.
pub const DEFAULT_FEATURE_DIM: usize = 384;

/// Extract deterministic, normalized numerical features from text.
///
/// This function uses a lightweight signed-hash projection:
/// - map each character and position into a feature slot
/// - accumulate signed values
/// - apply L2 normalization
pub fn extract_features(text: &str) -> Vec<f32> {
    extract_features_with_dim(text, DEFAULT_FEATURE_DIM)
}

/// Extract deterministic, normalized numerical features with a custom dimension.
pub fn extract_features_with_dim(text: &str, dim: usize) -> Vec<f32> {
    if dim == 0 {
        return Vec::new();
    }

    let mut features = vec![0.0_f32; dim];
    for (idx, ch) in text.chars().take(4096).enumerate() {
        let code = ch as u32 as usize;
        let slot = hash_slot(code, idx, dim);
        let sign = if ((code >> (idx % 16)) & 1) == 0 {
            1.0
        } else {
            -1.0
        };
        features[slot] += sign;
    }

    normalize_l2(&mut features);
    features
}

/// Extract feature vectors for a batch of texts.
pub fn extract_features_batch(texts: &[String], dim: usize) -> Vec<Vec<f32>> {
    texts
        .iter()
        .map(|text| extract_features_with_dim(text, dim))
        .collect()
}

fn hash_slot(code: usize, idx: usize, dim: usize) -> usize {
    let mut x = code.wrapping_add(0x9e3779b9usize);
    x ^= idx.wrapping_mul(0x85ebca6busize);
    x ^= x >> 16;
    x = x.wrapping_mul(0x27d4eb2dusize);
    x ^= x >> 15;
    x % dim
}

fn normalize_l2(values: &mut [f32]) {
    let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_features_is_deterministic() {
        let a = extract_features_with_dim("hello xenobot", 32);
        let b = extract_features_with_dim("hello xenobot", 32);
        assert_eq!(a, b);
    }

    #[test]
    fn extract_features_respects_dimension() {
        let v = extract_features_with_dim("abc", 16);
        assert_eq!(v.len(), 16);
    }

    #[test]
    fn extract_features_batch_returns_one_vector_per_text() {
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = extract_features_batch(&texts, 24);
        assert_eq!(out.len(), texts.len());
        assert!(out.iter().all(|row| row.len() == 24));
    }
}
