use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for analysis features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Enable NLP processing (Chinese segmentation, sentiment analysis, etc.)
    pub nlp_enabled: bool,

    /// Enable ML-based analysis (embedding generation, clustering, etc.)
    pub ml_enabled: bool,

    /// Enable GPU acceleration for ML models
    pub gpu_enabled: bool,

    /// Maximum number of worker threads for parallel processing
    pub max_workers: usize,

    /// Batch size for embedding generation
    pub batch_size: usize,

    /// Path to pre-trained models directory
    pub model_dir: PathBuf,

    /// NLP model configuration
    pub nlp: NlpConfig,

    /// ML model configuration
    pub ml: MlConfig,

    /// Embedding model configuration
    pub embedding: EmbeddingConfig,

    /// Sentiment analysis configuration
    pub sentiment: SentimentConfig,

    /// Topic modeling configuration
    pub topic_modeling: TopicModelingConfig,
}

/// NLP processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpConfig {
    /// Enable Chinese word segmentation
    pub chinese_segmentation: bool,

    /// Enable stopword filtering
    pub stopword_filtering: bool,

    /// Enable named entity recognition
    pub ner_enabled: bool,

    /// Enable part-of-speech tagging
    pub pos_tagging: bool,

    /// Minimum word length for processing
    pub min_word_length: usize,

    /// Maximum word length for processing
    pub max_word_length: usize,
}

/// Machine learning model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlConfig {
    /// Model name for embedding generation
    pub embedding_model: String,

    /// Model name for sentiment analysis
    pub sentiment_model: String,

    /// Model name for topic modeling
    pub topic_model: String,

    /// Model quantization (8-bit, 16-bit, 32-bit)
    pub quantization: String,

    /// Enable model caching
    pub cache_enabled: bool,

    /// Cache directory for models
    pub cache_dir: PathBuf,

    /// Maximum cache size in bytes
    pub max_cache_size: u64,
}

/// Embedding model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding dimension size
    pub dimension: usize,

    /// Context window size
    pub context_window: usize,

    /// Pooling method (mean, max, cls)
    pub pooling_method: String,

    /// Normalize embeddings
    pub normalize: bool,

    /// Enable gradient checkpointing for memory efficiency
    pub gradient_checkpointing: bool,
}

/// Sentiment analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentConfig {
    /// Number of sentiment classes (e.g., 2 for binary, 5 for fine-grained)
    pub num_classes: usize,

    /// Confidence threshold for predictions
    pub confidence_threshold: f32,

    /// Enable aspect-based sentiment analysis
    pub aspect_based: bool,

    /// Minimum text length for analysis
    pub min_text_length: usize,
}

/// Topic modeling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicModelingConfig {
    /// Number of topics to extract
    pub num_topics: usize,

    /// Topic modeling algorithm (lda, nmf, bertopic)
    pub algorithm: String,

    /// Number of top words per topic
    pub top_words_per_topic: usize,

    /// Minimum document frequency for words
    pub min_doc_frequency: f32,

    /// Maximum document frequency for words
    pub max_doc_frequency: f32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            nlp_enabled: true,
            ml_enabled: true,
            gpu_enabled: cfg!(target_os = "macos"),
            max_workers: num_cpus::get(),
            batch_size: 32,
            model_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("xenobot")
                .join("models"),
            nlp: NlpConfig::default(),
            ml: MlConfig::default(),
            embedding: EmbeddingConfig::default(),
            sentiment: SentimentConfig::default(),
            topic_modeling: TopicModelingConfig::default(),
        }
    }
}

impl Default for NlpConfig {
    fn default() -> Self {
        Self {
            chinese_segmentation: true,
            stopword_filtering: true,
            ner_enabled: true,
            pos_tagging: false,
            min_word_length: 1,
            max_word_length: 50,
        }
    }
}

impl Default for MlConfig {
    fn default() -> Self {
        Self {
            embedding_model: "BAAI/bge-small-zh-v1.5".to_string(),
            sentiment_model: "uer/roberta-base-finetuned-jd-binary-chinese".to_string(),
            topic_model: "allenai/specter2".to_string(),
            quantization: "16-bit".to_string(),
            cache_enabled: true,
            cache_dir: dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("xenobot")
                .join("model_cache"),
            max_cache_size: 2 * 1024 * 1024 * 1024, // 2GB
        }
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            dimension: 384,
            context_window: 512,
            pooling_method: "mean".to_string(),
            normalize: true,
            gradient_checkpointing: true,
        }
    }
}

impl Default for SentimentConfig {
    fn default() -> Self {
        Self {
            num_classes: 3, // negative, neutral, positive
            confidence_threshold: 0.7,
            aspect_based: false,
            min_text_length: 3,
        }
    }
}

impl Default for TopicModelingConfig {
    fn default() -> Self {
        Self {
            num_topics: 10,
            algorithm: "bertopic".to_string(),
            top_words_per_topic: 10,
            min_doc_frequency: 0.01,
            max_doc_frequency: 0.95,
        }
    }
}
