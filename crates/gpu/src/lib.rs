//! Metal MPS GPU acceleration for Xenobot (macOS arm64).
//!
//! This crate provides GPU-accelerated computations using Apple's Metal
//! Performance Shaders (MPS) framework for machine learning and data processing.

#![allow(unsafe_code)] // GPU operations require unsafe code for Metal buffer access

/// Metal backend abstraction.
pub mod metal;

/// MPS (Metal Performance Shaders) integration.
pub mod mps;

/// GPU-accelerated linear algebra operations.
pub mod linalg;

/// Neural network inference on GPU.
pub mod nn;

/// Error types for GPU operations.
pub mod error;

/// Configuration for GPU acceleration.
pub mod config;
