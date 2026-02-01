//! Session module for persisting scan results and user selections.
//!
//! This module provides functionality to save and load scan sessions,
//! allowing users to resume their work across different runs or on different
//! machines.
//!
//! # Features
//!
//! * **Persistence**: Save duplicate groups, scan settings, and user selections to JSON.
//! * **Integrity**: Each session file is wrapped in an envelope with a SHA256 checksum.
//! * **Versioning**: Supports versioned data formats to handle future schema changes.
//! * **Portability**: Files are stored in a human-readable JSON format.
//!
//! # Architecture
//!
//! * [`data`]: Serializable models for sessions, groups, and settings.
//! * [`io`]: Logic for saving, loading, and verifying session files.

pub mod data;
pub mod io;

pub use data::{Session, SessionGroup, SessionSettings, SESSION_VERSION};
