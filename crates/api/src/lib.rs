#![allow(dead_code)]

//! HTTP API server for Xenobot, replacing Electron IPC with Axum endpoints.
//!
//! This crate provides HTTP endpoints that mirror the IPC API surface from Xenobot's
//! Electron preload scripts, allowing the preserved Vue 3 frontend to work with
//! a Rust backend instead of Electron.

#![deny(unsafe_code)] // missing_docs temporarily disabled during development

pub mod config;
pub mod database;
pub mod error;
pub mod router;
pub mod server;
pub mod webhook_replay;

// API modules matching Xenobot's IPC API structure
pub mod agent;
pub mod ai;
pub mod cache;
pub mod chat;
pub mod core;
pub mod embedding;
pub mod events;
pub mod llm;
pub mod media;
pub mod merge;
pub mod network;
pub mod nlp;
pub mod secrets;
pub mod session;

pub use config::*;
pub use database::*;
pub use error::*;
pub use router::*;
pub use server::*;
