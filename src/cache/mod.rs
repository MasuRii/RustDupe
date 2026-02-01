//! Hash caching module for RustDupe.
//!
//! This module provides persistent storage for file hashes to speed up
//! subsequent scans by avoiding re-hashing of unchanged files.
//!
//! # Architecture
//!
//! The caching system is split into two main components:
//!
//! * [`database`]: Handles SQLite-based persistence, schema management, and CRUD operations.
//! * [`entry`]: Defines the data models stored in the cache and their validation logic.
//!
//! # Cache Invalidation
//!
//! Entries are validated using a combination of:
//! * File path (primary key)
//! * File size
//! * Modification time (mtime)
//! * Inode (as a secondary validation layer on supported platforms)
//!
//! If any of these attributes change, the cache entry is considered stale and
//! the file will be re-hashed during the next scan.

pub mod database;
pub mod entry;

pub use database::{CacheError, CacheResult, HashCache};
pub use entry::CacheEntry;
