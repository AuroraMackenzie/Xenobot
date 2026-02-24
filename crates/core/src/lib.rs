//! Core types, errors, and configuration for Xenobot
//!
//! This crate provides the foundational types and error handling used throughout
//! the Xenobot application, which combines Xenobot's multi-platform parsing
//! with chatlog's real-time WeChat extraction capabilities.

pub mod config;
pub mod constants;
pub mod error;
pub mod platform_sources;
pub mod types;
pub mod webhook;

// Re-exports for convenience
pub use config::XenobotConfig;
pub use error::{Error, Result};
pub use platform_sources::*;
pub use types::*;
