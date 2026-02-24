//! Real-time WeChat data extraction and decryption for Xenobot.
//!
//! This crate provides functionality for:
//! - Detecting running WeChat instances on macOS
//! - Managing user-supplied decryption keys with a legal-safe workflow
//! - Monitoring WeChat data directory for new database files
//! - Decrypting WeChat SQLite databases using authorized keys
//! - Providing real-time updates via events

#![deny(missing_docs)]
#![warn(unsafe_code)]
#![allow(dead_code)]

pub mod account;
pub mod audio;
pub mod config;
pub mod decrypt;
pub mod error;
pub mod media;
pub mod monitor;
pub mod service;

pub use error::{WeChatError, WeChatResult};
pub use service::WeChatService;
