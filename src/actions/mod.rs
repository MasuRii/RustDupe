//! File actions module.
//!
//! This module provides functionality for:
//! - Safe deletion via trash crate
//! - Permanent deletion (with confirmation)
//! - File preview (text, binary, image)
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
//!
//! # Preview
//!
//! The preview module supports file content preview:
//! - Text files: first 50 lines
//! - Binary files: hex dump of first 256 bytes
//! - Image files: metadata (format, dimensions, size)
//!
//! ```no_run
//! use rustdupe::actions::preview::preview_file_simple;
//! use std::path::Path;
//!
//! let content = preview_file_simple(Path::new("example.txt"));
//! println!("{}", content);
//! ```

pub mod delete;
pub mod preview;

// Re-export commonly used types
pub use delete::{
    delete_batch, delete_to_trash, delete_verified, permanent_delete, validate_preserves_copy,
    BatchDeleteResult, DeleteConfig, DeleteError, DeleteProgressCallback, DeleteResult,
    FileSnapshot,
};

pub use preview::{preview_file, preview_file_simple, PreviewContent, PreviewError, PreviewType};
