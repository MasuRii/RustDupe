//! File actions module.
//!
//! This module provides functionality for:
//! - Safe deletion via trash crate
//! - Permanent deletion (with confirmation)
//! - File preview
//!
//! # Deletion
//!
//! The delete module provides safe file deletion with:
//! - Move to system trash (default, recoverable)
//! - Permanent deletion (requires explicit configuration)
//! - Batch operations with progress reporting
//! - TOCTOU verification to detect modified files
//!
//! ```no_run
//! use rustdupe::actions::delete::{delete_to_trash, DeleteConfig};
//! use std::path::PathBuf;
//!
//! let path = PathBuf::from("/path/to/duplicate.txt");
//! let result = delete_to_trash(&path);
//! ```

pub mod delete;
pub mod preview;

// Re-export commonly used types
pub use delete::{
    delete_batch, delete_to_trash, delete_verified, permanent_delete, validate_preserves_copy,
    BatchDeleteResult, DeleteConfig, DeleteError, DeleteProgressCallback, DeleteResult,
    FileSnapshot,
};
