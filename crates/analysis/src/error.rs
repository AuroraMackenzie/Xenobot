use thiserror::Error;

/// Errors that can occur during analysis operations.
#[derive(Error, Debug)]
pub enum AnalysisError {
    /// NLP processing error.
    #[error("NLP error: {0}")]
    Nlp(String),
    /// Chat parsing error.
    #[error("Parse error: {0}")]
    Parse(String),
    /// Database operation error.
    #[error("Database error: {0}")]
    Database(String),
    /// AI/ML inference error.
    #[error("AI error: {0}")]
    Ai(String),
    /// Sentiment analysis error.
    #[error("Sentiment analysis error: {0}")]
    Sentiment(String),
    /// Entity recognition error.
    #[error("Entity recognition error: {0}")]
    EntityRecognition(String),
    /// Topic modeling error.
    #[error("Topic modeling error: {0}")]
    TopicModeling(String),
    /// Embedding generation error.
    #[error("Embedding generation error: {0}")]
    Embedding(String),
    /// Model loading error.
    #[error("Model loading error: {0}")]
    ModelLoading(String),
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),
    /// I/O operation error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// External API error.
    #[error("External API error: {0}")]
    ExternalApi(String),
    /// Invalid input data.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    /// Resource not found.
    #[error("Resource not found: {0}")]
    NotFound(String),
    /// Tokenization error.
    #[error("Tokenization error: {0}")]
    Tokenization(String),
    /// Vectorization error.
    #[error("Vectorization error: {0}")]
    Vectorization(String),
    /// Clustering error.
    #[error("Clustering error: {0}")]
    Clustering(String),
    /// Summarization error.
    #[error("Summarization error: {0}")]
    Summarization(String),
    /// Language detection error.
    #[error("Language detection error: {0}")]
    LanguageDetection(String),
    /// Time series analysis error.
    #[error("Time series analysis error: {0}")]
    TimeSeries(String),
    /// Pattern recognition error.
    #[error("Pattern recognition error: {0}")]
    PatternRecognition(String),
    /// Statistical analysis error.
    #[error("Statistical analysis error: {0}")]
    Statistical(String),
    /// GPU acceleration error.
    #[error("GPU acceleration error: {0}")]
    Gpu(String),
}

/// Result type alias for analysis operations.
pub type AnalysisResult<T> = Result<T, AnalysisError>;
