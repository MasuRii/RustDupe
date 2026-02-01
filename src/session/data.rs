//! Data structures for scan sessions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Current version of the session file format.
pub const SESSION_VERSION: u32 = 1;

/// Represents a saved scan session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Format version.
    pub version: u32,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// Root paths that were scanned.
    pub scan_paths: Vec<PathBuf>,
    /// Settings used during the scan.
    pub settings: SessionSettings,
    /// Duplicate groups found during the scan.
    pub groups: Vec<SessionGroup>,
    /// Paths selected by the user for deletion.
    pub user_selections: BTreeSet<PathBuf>,
}

impl Session {
    /// Create a new session with current timestamp and default version.
    pub fn new(
        scan_paths: Vec<PathBuf>,
        settings: SessionSettings,
        groups: Vec<SessionGroup>,
    ) -> Self {
        Self {
            version: SESSION_VERSION,
            created_at: Utc::now(),
            scan_paths,
            settings,
            groups,
            user_selections: BTreeSet::new(),
        }
    }
}

/// Settings used during the scan that produced the session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSettings {
    /// Follow symbolic links during traversal.
    pub follow_symlinks: bool,
    /// Skip hidden files and directories.
    pub skip_hidden: bool,
    /// Minimum file size to include (in bytes).
    pub min_size: Option<u64>,
    /// Maximum file size to include (in bytes).
    pub max_size: Option<u64>,
    /// Glob patterns to ignore.
    pub ignore_patterns: Vec<String>,
    /// Number of I/O threads used for hashing.
    pub io_threads: usize,
    /// Whether byte-by-byte verification was enabled.
    pub paranoid: bool,
}

/// A group of duplicates within a session.
///
/// Mirrors [`crate::duplicates::DuplicateGroup`] but designed for serialization
/// with additional metadata for session management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGroup {
    /// Unique identifier for the group in this session.
    pub id: usize,
    /// BLAKE3 hash of content.
    pub hash: [u8; 32],
    /// File size in bytes.
    pub size: u64,
    /// Paths to the duplicate files.
    pub files: Vec<PathBuf>,
}
