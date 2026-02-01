//! Data structures for scan sessions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::duplicates::{DuplicateGroup, ScanSummary};

/// Current version of the session file format.
pub const SESSION_VERSION: u32 = 2;

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
    /// Currently selected group index in TUI.
    pub group_index: usize,
    /// Currently selected file index in TUI.
    pub file_index: usize,
}

impl Session {
    /// Create a new session with current timestamp and default version.
    ///
    /// # Arguments
    ///
    /// * `scan_paths` - Root paths that were scanned
    /// * `settings` - Scan settings used
    /// * `groups` - Duplicate groups found
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
            group_index: 0,
            file_index: 0,
        }
    }

    /// Converts the session back to scan results (duplicate groups and summary).
    ///
    /// This allows the rest of the application to treat loaded sessions as
    /// if they were fresh scan results.
    ///
    /// # Returns
    ///
    /// A tuple containing the list of duplicate groups and a calculated scan summary.
    #[must_use]
    pub fn to_results(&self) -> (Vec<DuplicateGroup>, ScanSummary) {
        let groups: Vec<DuplicateGroup> = self.groups.iter().cloned().map(Into::into).collect();

        let summary = ScanSummary {
            duplicate_groups: groups.len(),
            duplicate_files: groups.iter().map(|g| g.duplicate_count()).sum(),
            reclaimable_space: groups.iter().map(|g| g.wasted_space()).sum(),
            // Total files and size are not fully known from session alone,
            // so we provide estimates based on duplicate groups.
            total_files: groups.iter().map(|g| g.files.len()).sum(),
            total_size: groups
                .iter()
                .map(|g| g.files.iter().map(|f| f.size).sum::<u64>())
                .sum(),
            ..ScanSummary::default()
        };

        (groups, summary)
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
    /// Only include files modified after this date.
    pub newer_than: Option<DateTime<Utc>>,
    /// Only include files modified before this date.
    pub older_than: Option<DateTime<Utc>>,
    /// Glob patterns to ignore.
    pub ignore_patterns: Vec<String>,
    /// Regex patterns to include.
    pub regex_include: Vec<String>,
    /// Regex patterns to exclude.
    pub regex_exclude: Vec<String>,
    /// File categories to include.
    pub file_categories: Vec<crate::scanner::FileCategory>,
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
    /// Detailed file information for each duplicate.
    pub files: Vec<crate::scanner::FileEntry>,
    /// Protected reference paths.
    #[serde(default)]
    pub reference_paths: Vec<PathBuf>,
}

impl SessionGroup {
    /// Creates a session group from a duplicate group.
    ///
    /// # Arguments
    ///
    /// * `group` - The original duplicate group
    /// * `id` - A unique identifier for this group in the session
    #[must_use]
    pub fn from_duplicate_group(group: &DuplicateGroup, id: usize) -> Self {
        Self {
            id,
            hash: group.hash,
            size: group.size,
            files: group.files.clone(),
            reference_paths: group.reference_paths.clone(),
        }
    }
}

impl From<SessionGroup> for DuplicateGroup {
    fn from(sg: SessionGroup) -> Self {
        DuplicateGroup::new(sg.hash, sg.size, sg.files, sg.reference_paths)
    }
}
