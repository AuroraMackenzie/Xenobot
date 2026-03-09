//! Core types, errors, and configuration for Xenobot
//!
//! This crate provides the foundational types and error handling used throughout
//! the Xenobot application, including multi-platform parsing and
//! legal-safe local ingestion capabilities.

pub mod config;
pub mod constants;
pub mod error;
pub mod sandbox;
pub mod platform_sources;
pub mod types;
pub mod webhook;

// Re-exports for convenience
pub use config::XenobotConfig;
pub use error::{Error, Result};
pub use platform_sources::*;
pub use sandbox::*;
pub use types::*;
