//! AI-powered analysis and multi-platform chat parsing for Xenobot.
//!
//! This crate provides natural language processing, machine learning,
//! and multi-platform chat format parsing capabilities.

#![deny(missing_docs, unsafe_code)]
#![allow(dead_code)]

/// Natural language processing utilities.
pub mod nlp;

/// Machine learning models and inference.
pub mod ml;

/// Multi-platform chat format parsers.
pub mod parsers;

/// Feature extraction and vectorization.
pub mod features;

/// Error types for analysis operations.
pub mod error;

/// Configuration for analysis modules.
pub mod config;
