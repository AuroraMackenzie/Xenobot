//! Error types for GPU operations.

use thiserror::Error;

/// Main error type for GPU operations.
#[derive(Error, Debug)]
pub enum GpuError {
    /// Metal API error.
    #[error("Metal error: {0}")]
    Metal(String),

    /// MPS (Metal Performance Shaders) error.
    #[error("MPS error: {0}")]
    Mps(String),

    /// Linear algebra operation error.
    #[error("Linear algebra error: {0}")]
    LinearAlgebra(String),
    /// Neural network operation error.
    #[error("Neural network error: {0}")]
    NeuralNetwork(String),

    /// Candle tensor error.
    #[error("Candle error: {0}")]
    Candle(String),

    /// Invalid configuration.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Unsupported operation.
    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    /// Internal GPU error.
    #[error("Internal GPU error: {0}")]
    Internal(String),
}

/// Result alias for GPU operations.
pub type Result<T> = std::result::Result<T, GpuError>;
