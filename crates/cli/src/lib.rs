//! Command-line interface for Xenobot.
//!
//! This crate provides CLI commands for managing Xenobot instances,
//! importing/exporting data, and interacting with the API.

#![deny(missing_docs, unsafe_code)]

/// CLI command definitions and parsing.
pub mod commands;

/// CLI application entry point and configuration.
pub mod app;

/// Error types for CLI operations.
pub mod error;
