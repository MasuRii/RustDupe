//! Scanner module for directory traversal and file hashing.
//!
//! This module provides functionality for:
//! - Parallel directory walking using jwalk
//! - Content hashing with BLAKE3
//! - Hardlink detection
//! - Unicode path normalization

pub mod hasher;
pub mod walker;
