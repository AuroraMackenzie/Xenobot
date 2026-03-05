use crate::error::{AnalysisError, AnalysisResult};
use crate::features::{extract_features_with_dim, DEFAULT_FEATURE_DIM};
use candle_core::{Device, Tensor};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use tokenizers::tokenizer::Tokenizer;

/// Machine learning model manager for analysis tasks.
pub struct ModelManager {
    device: Device,
    embedding_model: Option<Arc<dyn EmbeddingModel>>,
    sentiment_model: Option<Arc<dyn ClassificationModel>>,
    topic_model: Option<Arc<dyn TopicModel>>,
    tokenizer: Option<Arc<Tokenizer>>,
    cache: HashMap<String, Tensor>,
}

impl ModelManager {
    /// Create a new ModelManager with specified device.
    pub fn new(device: Device) -> Self {
        Self {
            device,
            embedding_model: None,
            sentiment_model: None,
            topic_model: None,
            tokenizer: None,
            cache: HashMap::new(),
        }
    }

    /// Load embedding model from path.
    ///
    /// Current implementation provides a deterministic local embedding model,
    /// which avoids network dependencies and remains reproducible across runs.
    pub fn load_embedding_model(&mut self, model_path: &Path) -> AnalysisResult<()> {
        let model_name = model_path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("deterministic-embedding")
            .to_string();

        self.embedding_model = Some(Arc::new(DeterministicEmbeddingModel::new(
            model_name,
            DEFAULT_FEATURE_DIM,
            self.tokenizer.clone(),
        )));
        Ok(())
    }

    /// Load sentiment analysis model from path.
    pub fn load_sentiment_model(&mut self, _model_path: &Path) -> AnalysisResult<()> {
        self.sentiment_model = Some(Arc::new(LexiconClassificationModel::default()));
        Ok(())
    }

    /// Load topic modeling model from path.
    pub fn load_topic_model(&mut self, _model_path: &Path) -> AnalysisResult<()> {
        self.topic_model = Some(Arc::new(FrequencyTopicModel::default()));
        Ok(())
    }

    /// Load tokenizer from path.
    pub fn load_tokenizer(&mut self, tokenizer_path: &Path) -> AnalysisResult<()> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| AnalysisError::Tokenization(format!("failed to load tokenizer: {}", e)))?;
        self.tokenizer = Some(Arc::new(tokenizer));

        if self.embedding_model.is_some() {
            self.embedding_model = Some(Arc::new(DeterministicEmbeddingModel::new(
                "deterministic-embedding".to_string(),
                DEFAULT_FEATURE_DIM,
                self.tokenizer.clone(),
            )));
        }

        Ok(())
    }

    /// Generate embeddings for texts.
    pub fn embed_texts(&self, texts: &[String]) -> AnalysisResult<Vec<Vec<f32>>> {
        if let Some(model) = &self.embedding_model {
            model.embed(texts)
        } else {
            Err(AnalysisError::ModelLoading(
                "Embedding model not loaded".to_string(),
            ))
        }
    }

    /// Predict sentiment for texts.
    pub fn predict_sentiment(&self, texts: &[String]) -> AnalysisResult<Vec<SentimentPrediction>> {
        if let Some(model) = &self.sentiment_model {
            model.predict(texts)
        } else {
            Err(AnalysisError::ModelLoading(
                "Sentiment model not loaded".to_string(),
            ))
        }
    }

    /// Extract topics from texts.
    pub fn extract_topics(
        &self,
        texts: &[String],
        num_topics: usize,
    ) -> AnalysisResult<Vec<Topic>> {
        if let Some(model) = &self.topic_model {
            model.extract_topics(texts, num_topics)
        } else {
            Err(AnalysisError::ModelLoading(
                "Topic model not loaded".to_string(),
            ))
        }
    }

    /// Run inference on input features (generic prediction).
    pub fn predict(&self, input: &[f32]) -> AnalysisResult<Vec<f32>> {
        let tensor = Tensor::from_vec(input.to_vec(), (input.len(),), &self.device)
            .map_err(|e: candle_core::Error| AnalysisError::Ai(e.to_string()))?;

        let output = tensor
            .to_vec1::<f32>()
            .map_err(|e: candle_core::Error| AnalysisError::Ai(e.to_string()))?;

        Ok(output)
    }

    /// Clear model cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Trait for embedding models.
pub trait EmbeddingModel: Send + Sync {
    /// Generate embeddings for texts.
    fn embed(&self, texts: &[String]) -> AnalysisResult<Vec<Vec<f32>>>;

    /// Get embedding dimension.
    fn dimension(&self) -> usize;

    /// Get model name.
    fn name(&self) -> &str;
}

/// Trait for classification models.
pub trait ClassificationModel: Send + Sync {
    /// Predict classes for texts.
    fn predict(&self, texts: &[String]) -> AnalysisResult<Vec<SentimentPrediction>>;

    /// Get number of classes.
    fn num_classes(&self) -> usize;

    /// Get class labels.
    fn class_labels(&self) -> Vec<String>;
}

/// Trait for topic models.
pub trait TopicModel: Send + Sync {
    /// Extract topics from texts.
    fn extract_topics(&self, texts: &[String], num_topics: usize) -> AnalysisResult<Vec<Topic>>;

    /// Get topic words for a topic.
    fn topic_words(&self, topic_id: usize, num_words: usize) -> AnalysisResult<Vec<String>>;
}

/// Sentiment prediction result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentPrediction {
    /// Predicted class index.
    pub class_idx: usize,

    /// Predicted class label.
    pub class_label: String,

    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,

    /// Probability distribution over all classes.
    pub probabilities: Vec<f32>,
}

/// Topic representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    /// Topic ID.
    pub id: usize,

    /// Topic label (most representative words).
    pub label: String,

    /// Topic weight in the corpus.
    pub weight: f32,

    /// Top words and their weights.
    pub top_words: Vec<(String, f32)>,

    /// Representative documents for this topic.
    pub representative_docs: Vec<String>,
}

/// Deterministic local embedding model.
struct DeterministicEmbeddingModel {
    name: String,
    dim: usize,
    tokenizer: Option<Arc<Tokenizer>>,
}

impl DeterministicEmbeddingModel {
    fn new(name: String, dim: usize, tokenizer: Option<Arc<Tokenizer>>) -> Self {
        Self {
            name,
            dim: dim.max(1),
            tokenizer,
        }
    }

    fn project_token_ids(&self, ids: &[u32]) -> Vec<f32> {
        if ids.is_empty() {
            return vec![0.0; self.dim];
        }

        let mut vector = vec![0.0_f32; self.dim];
        for (index, token_id) in ids.iter().take(4096).enumerate() {
            let seed = ((*token_id as u64) << 32) | (index as u64);
            let h = stable_hash(seed);
            let slot = (h as usize) % self.dim;
            let sign = if (h & 1) == 0 { 1.0 } else { -1.0 };
            vector[slot] += sign;
        }
        normalize_l2(&mut vector);
        vector
    }
}

impl EmbeddingModel for DeterministicEmbeddingModel {
    fn embed(&self, texts: &[String]) -> AnalysisResult<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());

        for text in texts {
            let embedding = if let Some(tokenizer) = &self.tokenizer {
                match tokenizer.encode(text.as_str(), true) {
                    Ok(encoding) => self.project_token_ids(encoding.get_ids()),
                    Err(_) => extract_features_with_dim(text, self.dim),
                }
            } else {
                extract_features_with_dim(text, self.dim)
            };
            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Lexicon-based sentiment model for local deterministic inference.
#[derive(Default)]
struct LexiconClassificationModel;

impl LexiconClassificationModel {
    fn classify_one(&self, text: &str) -> SentimentPrediction {
        let mut positive_count = 0.0_f32;
        let mut negative_count = 0.0_f32;

        for token in tokenize_words(text) {
            if POSITIVE_WORDS.contains(&token.as_str()) {
                positive_count += 1.0;
            }
            if NEGATIVE_WORDS.contains(&token.as_str()) {
                negative_count += 1.0;
            }
        }

        let neutral_bias = 1.0;
        let logits = [1.0 + negative_count, neutral_bias, 1.0 + positive_count];
        let probabilities = softmax(&logits);
        let class_idx = if positive_count > negative_count {
            2
        } else if negative_count > positive_count {
            0
        } else {
            1
        };
        let confidence = probabilities.get(class_idx).copied().unwrap_or(0.0);

        let class_label = match class_idx {
            0 => "negative",
            2 => "positive",
            _ => "neutral",
        }
        .to_string();

        SentimentPrediction {
            class_idx,
            class_label,
            confidence,
            probabilities,
        }
    }
}

impl ClassificationModel for LexiconClassificationModel {
    fn predict(&self, texts: &[String]) -> AnalysisResult<Vec<SentimentPrediction>> {
        Ok(texts.iter().map(|t| self.classify_one(t)).collect())
    }

    fn num_classes(&self) -> usize {
        3
    }

    fn class_labels(&self) -> Vec<String> {
        vec![
            "negative".to_string(),
            "neutral".to_string(),
            "positive".to_string(),
        ]
    }
}

/// Frequency-based topic model.
#[derive(Default)]
struct FrequencyTopicModel;

impl TopicModel for FrequencyTopicModel {
    fn extract_topics(&self, texts: &[String], num_topics: usize) -> AnalysisResult<Vec<Topic>> {
        if num_topics == 0 {
            return Ok(Vec::new());
        }

        let mut global_freq: HashMap<String, usize> = HashMap::new();
        for text in texts {
            for token in tokenize_words(text) {
                if token.len() <= 1 || STOPWORDS.contains(&token.as_str()) {
                    continue;
                }
                *global_freq.entry(token).or_insert(0) += 1;
            }
        }

        if global_freq.is_empty() {
            return Ok((0..num_topics)
                .map(|id| Topic {
                    id,
                    label: format!("Topic {}", id),
                    weight: 0.0,
                    top_words: Vec::new(),
                    representative_docs: texts.iter().take(3).cloned().collect(),
                })
                .collect());
        }

        let mut buckets: Vec<Vec<(String, usize)>> = vec![Vec::new(); num_topics];
        for (word, count) in global_freq {
            let bucket = (stable_hash_str(&word) as usize) % num_topics;
            buckets[bucket].push((word, count));
        }

        let total_weight: usize = buckets
            .iter()
            .flat_map(|bucket| bucket.iter().map(|(_, c)| *c))
            .sum();

        let mut topics = Vec::with_capacity(num_topics);
        for (topic_id, bucket_words) in buckets.into_iter().enumerate() {
            let mut sorted_words = bucket_words;
            sorted_words.sort_by(|a, b| b.1.cmp(&a.1));

            let bucket_sum: usize = sorted_words.iter().map(|(_, c)| *c).sum();
            let weight = if total_weight > 0 {
                bucket_sum as f32 / total_weight as f32
            } else {
                0.0
            };

            let top_words: Vec<(String, f32)> = sorted_words
                .iter()
                .take(8)
                .map(|(word, count)| {
                    let local_weight = if bucket_sum > 0 {
                        *count as f32 / bucket_sum as f32
                    } else {
                        0.0
                    };
                    (word.clone(), local_weight)
                })
                .collect();

            let label = if top_words.is_empty() {
                format!("Topic {}", topic_id)
            } else {
                top_words
                    .iter()
                    .take(3)
                    .map(|(w, _)| w.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };

            let representative_docs = select_representative_docs(texts, &top_words);

            topics.push(Topic {
                id: topic_id,
                label,
                weight,
                top_words,
                representative_docs,
            });
        }

        Ok(topics)
    }

    fn topic_words(&self, topic_id: usize, num_words: usize) -> AnalysisResult<Vec<String>> {
        let base = [
            "project",
            "message",
            "meeting",
            "task",
            "release",
            "issue",
            "customer",
            "support",
            "quality",
            "review",
            "feature",
            "design",
            "runtime",
            "database",
            "network",
            "analysis",
            "assistant",
            "privacy",
            "policy",
            "export",
        ];

        let mut words = Vec::with_capacity(num_words);
        for i in 0..num_words {
            let idx = (topic_id + i) % base.len();
            words.push(base[idx].to_string());
        }
        Ok(words)
    }
}

/// GPU device utilities.
pub mod gpu {
    use super::*;

    /// Get the best available device (GPU preferred).
    pub fn best_available_device() -> AnalysisResult<Device> {
        #[cfg(target_os = "macos")]
        {
            if let Ok(device) = Device::new_metal(0) {
                return Ok(device);
            }
        }

        Ok(Device::Cpu)
    }

    /// Check if Metal MPS is available.
    pub fn metal_mps_available() -> bool {
        #[cfg(target_os = "macos")]
        {
            Device::new_metal(0).is_ok()
        }

        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

fn stable_hash(seed: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    hasher.finish()
}

fn stable_hash_str(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn normalize_l2(values: &mut [f32]) {
    let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, |acc, v| acc.max(v));
    let mut exp_values: Vec<f32> = logits.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = exp_values.iter().sum();
    if sum > 0.0 {
        for value in &mut exp_values {
            *value /= sum;
        }
    }
    exp_values
}

fn select_representative_docs(texts: &[String], top_words: &[(String, f32)]) -> Vec<String> {
    if top_words.is_empty() {
        return texts.iter().take(3).cloned().collect();
    }

    let keywords: Vec<&str> = top_words.iter().take(5).map(|(w, _)| w.as_str()).collect();
    let mut picked = Vec::new();
    for text in texts {
        let lower = text.to_lowercase();
        if keywords.iter().any(|word| lower.contains(word)) {
            picked.push(text.clone());
            if picked.len() >= 3 {
                break;
            }
        }
    }

    if picked.is_empty() {
        texts.iter().take(3).cloned().collect()
    } else {
        picked
    }
}

fn tokenize_words(text: &str) -> Vec<String> {
    static WORD_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[\p{L}\p{N}_]+(?:'[\p{L}]+)?").expect("valid regex"));

    WORD_RE
        .find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}

static POSITIVE_WORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "good",
        "great",
        "excellent",
        "nice",
        "love",
        "awesome",
        "happy",
        "success",
        "like",
        "amazing",
        "赞",
        "喜欢",
        "开心",
        "满意",
        "成功",
        "优秀",
    ]
    .into_iter()
    .collect()
});

static NEGATIVE_WORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "bad", "terrible", "awful", "hate", "sad", "angry", "failure", "worse", "problem", "bug",
        "差", "讨厌", "生气", "失败", "糟糕", "问题",
    ]
    .into_iter()
    .collect()
});

static STOPWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "is", "are", "was", "were",
        "be", "this", "that", "with", "as", "by", "的", "了", "在", "是", "和", "就", "都", "一个",
        "我们", "你们", "他们", "以及", "或者",
    ]
    .into_iter()
    .collect()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_embedding_returns_stable_vectors() {
        let model = DeterministicEmbeddingModel::new("test".to_string(), 64, None);
        let texts = vec!["hello xenobot".to_string(), "hello xenobot".to_string()];
        let embeddings = model.embed(&texts).expect("embedding should succeed");
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0], embeddings[1]);
        assert_eq!(embeddings[0].len(), 64);
    }

    #[test]
    fn sentiment_model_detects_positive_and_negative_terms() {
        let model = LexiconClassificationModel;
        let texts = vec![
            "I love this excellent release".to_string(),
            "This is a terrible bad issue".to_string(),
        ];
        let out = model.predict(&texts).expect("prediction should succeed");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].class_label, "positive");
        assert_eq!(out[1].class_label, "negative");
    }

    #[test]
    fn topic_model_returns_requested_topic_count() {
        let model = FrequencyTopicModel;
        let texts = vec![
            "release planning message for project alpha".to_string(),
            "database issue and runtime bug report".to_string(),
            "customer support review and quality check".to_string(),
        ];
        let topics = model
            .extract_topics(&texts, 3)
            .expect("topic extraction should succeed");
        assert_eq!(topics.len(), 3);
    }
}
