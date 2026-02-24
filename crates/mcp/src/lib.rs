//! MCP (Model Context Protocol) integration for Xenobot.
//!
//! This crate provides MCP server and client implementations for integrating
//! Xenobot with AI assistants and other MCP-compatible tools.

#![deny(missing_docs, unsafe_code)]

/// MCP server implementation.
pub mod server;

/// MCP client implementation.
pub mod client;

/// Protocol definitions and utilities.
pub mod protocol;

/// Error types for MCP operations.
pub mod error;

/// Configuration for MCP integration.
pub mod config;
