//! Web frontend integration for Xenobot (preserving Vue 3/TypeScript frontend).
//!
//! This crate serves static frontend files and provides WebSocket endpoints
//! for real-time updates and API integration.

#![deny(unsafe_code)] // missing_docs temporarily disabled during development

/// Static file serving and asset management.
pub mod assets;

/// WebSocket server for real-time updates.
pub mod websocket;

/// Frontend integration utilities.
pub mod integration;

/// Error types for web operations.
pub mod error;
