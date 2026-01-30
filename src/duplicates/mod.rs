//! Duplicate detection module.
//!
//! This module provides functionality for:
//! - Size-based file grouping (Phase 1)
//! - Prehash comparison (Phase 2)
//! - Full hash comparison (Phase 3)
//! - Duplicate group management
//!
//! # Architecture
//!
//! Duplicate detection uses a multi-phase pipeline to efficiently find
//! identical files:
//!
//! 1. **Phase 1 - Size Grouping**: Group files by size. Files with unique
//!    sizes cannot be duplicates and are eliminated. This typically removes
//!    70-90% of files from consideration.
//!
//! 2. **Phase 2 - Prehash**: For files with matching sizes, compute a hash
//!    of the first 4KB. This quickly eliminates files that differ early.
//!
//! 3. **Phase 3 - Full Hash**: For files with matching prehashes, compute
//!    the full content hash to confirm they are true duplicates.
//!
//! 4. **Phase 4 - Verification** (optional): Byte-by-byte comparison for
//!    paranoid mode.
//!
//! # Example
//!
//! ```
//! use rustdupe::scanner::{Walker, WalkerConfig, FileEntry};
//! use rustdupe::duplicates::{group_by_size, SizeGroup, DuplicateGroup, GroupingStats};
//! use std::path::Path;
//!
//! // Phase 1: Collect files and group by size
//! // let walker = Walker::new(Path::new("."), WalkerConfig::default());
//! // let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();
//! // let (size_groups, stats) = group_by_size(files);
//! //
//! // println!("Phase 1: {} files → {} potential duplicates",
//! //     stats.total_files, stats.potential_duplicates);
//! ```

pub mod finder;
pub mod groups;

// Re-export main types from groups
pub use groups::{
    group_by_size, group_by_size_structured, DuplicateGroup, GroupingStats, SizeGroup,
};
