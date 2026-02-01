//! Hash caching module for RustDupe.
//!
//! This module provides persistent storage for file hashes to speed up
//! subsequent scans by avoiding re-hashing of unchanged files.

pub mod database;
pub mod entry;

pub use database::HashCache;
pub use entry::CacheEntry;
