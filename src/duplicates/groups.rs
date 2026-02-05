//! Duplicate grouping and size-based file organization.
//!
//! # Overview
//!
//! This module provides structures for grouping files by size (Phase 1 of
//! duplicate detection) and managing duplicate groups for later phases.
//!
//! ## Size Grouping (Phase 1)
//!
//! Size grouping is the first phase of duplicate detection. It groups files
//! by their exact size, eliminating 70-90% of non-duplicates instantly since
//! files with different sizes cannot be duplicates.
//!
//! # Example
//!
//! ```
//! use rustdupe::scanner::FileEntry;
//! use rustdupe::duplicates::{group_by_size, GroupingStats};
//! use std::path::PathBuf;
//! use std::time::SystemTime;
//!
//! // Create some file entries
//! let files = vec![
//!     FileEntry::new(PathBuf::from("/file1.txt"), 1024, SystemTime::now()),
//!     FileEntry::new(PathBuf::from("/file2.txt"), 1024, SystemTime::now()),
//!     FileEntry::new(PathBuf::from("/file3.txt"), 2048, SystemTime::now()),
//! ];
//!
//! // Group by size - only groups with 2+ files are potential duplicates
//! let (groups, stats) = group_by_size(files);
//!
//! assert_eq!(stats.total_files, 3);
//! assert_eq!(stats.potential_duplicates, 2);  // Two 1024-byte files
//! assert_eq!(groups.len(), 1);  // Only one size group with multiple files
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::scanner::FileEntry;

/// A group of files with the same size.
///
/// Used in Phase 1 of duplicate detection to organize files before
/// comparing content hashes.
#[derive(Debug, Clone)]
pub struct SizeGroup {
    /// File size in bytes (shared by all files in this group)
    pub size: u64,
    /// Files with this exact size
    pub files: Vec<FileEntry>,
}

impl SizeGroup {
    /// Create a new size group.
    ///
    /// # Arguments
    ///
    /// * `size` - The file size for this group
    #[must_use]
    pub fn new(size: u64) -> Self {
        Self {
            size,
            files: Vec::new(),
        }
    }

    /// Create a size group with initial files.
    ///
    /// # Arguments
    ///
    /// * `size` - The file size for this group
    /// * `files` - Initial files in the group
    #[must_use]
    pub fn with_files(size: u64, files: Vec<FileEntry>) -> Self {
        Self { size, files }
    }

    /// Add a file to this group.
    ///
    /// # Panics
    ///
    /// Debug assertion fails if file size doesn't match group size.
    pub fn add(&mut self, file: FileEntry) {
        debug_assert_eq!(
            file.size, self.size,
            "File size {} doesn't match group size {}",
            file.size, self.size
        );
        self.files.push(file);
    }

    /// Number of files in this group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if this group is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Check if this group has potential duplicates (2+ files).
    #[must_use]
    pub fn has_duplicates(&self) -> bool {
        self.files.len() > 1
    }

    /// Total size of all files in this group.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.size * self.files.len() as u64
    }

    /// Potential space savings (all copies minus one).
    #[must_use]
    pub fn potential_savings(&self) -> u64 {
        if self.files.len() > 1 {
            self.size * (self.files.len() as u64 - 1)
        } else {
            0
        }
    }
}

/// Confirmed duplicate group of files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    /// BLAKE3 hash of the file content (32 bytes)
    /// For similar groups, this is the perceptual hash of the first file.
    pub hash: [u8; 32],
    /// File size in bytes (shared by all files in exact groups, varied in similar)
    pub size: u64,
    /// Detailed file information for each duplicate
    pub files: Vec<FileEntry>,
    /// Protected reference paths
    pub reference_paths: Vec<std::path::PathBuf>,
    /// Whether this is a similarity-based group rather than an exact duplicate
    #[serde(default)]
    pub is_similar: bool,
}

impl DuplicateGroup {
    /// Create a new duplicate group.
    ///
    /// # Arguments
    ///
    /// * `hash` - BLAKE3 content hash
    /// * `size` - File size in bytes
    /// * `files` - Detailed file entries
    /// * `reference_paths` - Protected reference paths
    #[must_use]
    pub fn new(
        hash: [u8; 32],
        size: u64,
        files: Vec<FileEntry>,
        reference_paths: Vec<std::path::PathBuf>,
    ) -> Self {
        Self {
            hash,
            size,
            files,
            reference_paths,
            is_similar: false,
        }
    }

    /// Create a new similar image group.
    #[must_use]
    pub fn new_similar(
        id_hash: [u8; 32],
        files: Vec<FileEntry>,
        reference_paths: Vec<std::path::PathBuf>,
    ) -> Self {
        let size = files.first().map_or(0, |f| f.size);
        Self {
            hash: id_hash,
            size,
            files,
            reference_paths,
            is_similar: true,
        }
    }

    /// Number of files in this group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if this group is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Total size of all files in this group.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Total wasted space (all copies minus one).
    #[must_use]
    pub fn wasted_space(&self) -> u64 {
        if self.files.len() > 1 {
            self.total_size().saturating_sub(self.files[0].size)
        } else {
            0
        }
    }

    /// Number of duplicate copies (total - 1 original).
    #[must_use]
    pub fn duplicate_count(&self) -> usize {
        self.files.len().saturating_sub(1)
    }

    /// Hash as hexadecimal string.
    #[must_use]
    pub fn hash_hex(&self) -> String {
        crate::scanner::hash_to_hex(&self.hash)
    }

    /// Get just the paths of files in this group.
    #[must_use]
    pub fn paths(&self) -> Vec<std::path::PathBuf> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }

    /// Check if a path is in a protected reference directory.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check
    #[must_use]
    pub fn is_in_reference_dir(&self, path: &std::path::Path) -> bool {
        self.reference_paths.iter().any(|ref_path| {
            if cfg!(windows) {
                // Windows is case-insensitive. Convert to lowercase PathBuf for reliable
                // component-based comparison.
                let p = std::path::PathBuf::from(path.to_string_lossy().to_lowercase());
                let r = std::path::PathBuf::from(ref_path.to_string_lossy().to_lowercase());
                p.starts_with(r)
            } else {
                path.starts_with(ref_path)
            }
        })
    }
}

/// Statistics from size grouping phase.
///
/// Provides insight into the distribution of files by size and
/// the effectiveness of the size grouping filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupingStats {
    /// Total number of files processed
    pub total_files: usize,
    /// Total size of all files in bytes
    pub total_size: u64,
    /// Number of unique file sizes
    pub unique_sizes: usize,
    /// Number of files that could be duplicates (in groups of 2+)
    pub potential_duplicates: usize,
    /// Number of files eliminated as unique (singleton groups)
    pub eliminated_unique: usize,
    /// Number of empty files encountered (size 0, handled separately)
    pub empty_files: usize,
    /// Number of size groups with 2+ files (potential duplicate groups)
    pub duplicate_groups: usize,
}

impl GroupingStats {
    /// Percentage of files eliminated by size grouping.
    #[must_use]
    pub fn elimination_rate(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.eliminated_unique as f64 / self.total_files as f64) * 100.0
        }
    }

    /// Potential space savings if all duplicates were removed.
    #[must_use]
    pub fn max_potential_savings(&self, groups: &HashMap<u64, Vec<FileEntry>>) -> u64 {
        groups
            .values()
            .filter(|files| files.len() > 1)
            .map(|files| {
                let size = files.first().map_or(0, |f| f.size);
                size * (files.len() as u64 - 1)
            })
            .sum()
    }
}

/// Group files by size (Phase 1 of duplicate detection).
///
/// This is the first phase of duplicate detection. It groups all files by their
/// exact size, since files with different sizes cannot be duplicates. This
/// typically eliminates 70-90% of files from further consideration.
///
/// # Arguments
///
/// * `files` - Iterator of file entries to group
///
/// # Returns
///
/// A tuple of:
/// - `HashMap<u64, Vec<FileEntry>>` - Files grouped by size (only groups with 2+ files)
/// - `GroupingStats` - Statistics about the grouping operation
///
/// # Performance
///
/// - Time complexity: O(n) where n is the number of files
/// - Space complexity: O(n) for storing file entries
/// - No file I/O is performed (metadata only)
///
/// # Example
///
/// ```
/// use rustdupe::scanner::FileEntry;
/// use rustdupe::duplicates::group_by_size;
/// use std::path::PathBuf;
/// use std::time::SystemTime;
///
/// let files = vec![
///     FileEntry::new(PathBuf::from("/a.txt"), 100, SystemTime::now()),
///     FileEntry::new(PathBuf::from("/b.txt"), 100, SystemTime::now()),
///     FileEntry::new(PathBuf::from("/c.txt"), 200, SystemTime::now()),
/// ];
///
/// let (groups, stats) = group_by_size(files);
///
/// // Only the 100-byte group is returned (has 2 files)
/// assert_eq!(groups.len(), 1);
/// assert!(groups.contains_key(&100));
/// assert_eq!(groups[&100].len(), 2);
///
/// // Stats show filtering effectiveness
/// assert_eq!(stats.total_files, 3);
/// assert_eq!(stats.eliminated_unique, 1);  // The 200-byte file
/// ```
#[must_use]
pub fn group_by_size(
    files: impl IntoIterator<Item = FileEntry>,
) -> (HashMap<u64, Vec<FileEntry>>, GroupingStats) {
    let mut all_groups: HashMap<u64, Vec<FileEntry>> = HashMap::new();
    let mut stats = GroupingStats::default();
    let mut empty_files_seen = 0u64;

    // First pass: group all files by size
    for file in files {
        stats.total_files += 1;
        stats.total_size += file.size;

        // Handle empty files separately
        if file.size == 0 {
            empty_files_seen += 1;
            log::debug!("Empty file encountered: {}", file.path.display());
            continue;
        }

        all_groups.entry(file.size).or_default().push(file);
    }

    stats.empty_files = empty_files_seen as usize;

    // Log warning if empty files were found
    if empty_files_seen > 0 {
        log::warn!(
            "Skipped {} empty file(s) - all empty files have identical hash",
            empty_files_seen
        );
    }

    // Record unique sizes before filtering
    stats.unique_sizes = all_groups.len();

    // Second pass: filter to only groups with 2+ files
    let filtered_groups: HashMap<u64, Vec<FileEntry>> = all_groups
        .into_iter()
        .filter(|(size, files)| {
            if files.len() == 1 {
                stats.eliminated_unique += 1;
                log::trace!(
                    "Eliminated unique size {}: {}",
                    size,
                    files[0].path.display()
                );
                false
            } else {
                stats.potential_duplicates += files.len();
                stats.duplicate_groups += 1;
                log::debug!(
                    "Size group {} bytes: {} potential duplicates",
                    size,
                    files.len()
                );
                true
            }
        })
        .collect();

    log::info!(
        "Phase 1 complete: {} files → {} potential duplicates ({:.1}% eliminated)",
        stats.total_files,
        stats.potential_duplicates,
        stats.elimination_rate()
    );

    (filtered_groups, stats)
}

/// Group files by size, returning SizeGroup structs.
///
/// Alternative to `group_by_size` that returns `SizeGroup` structs
/// instead of a raw HashMap. Useful when you need the additional
/// methods on `SizeGroup`.
///
/// # Arguments
///
/// * `files` - Iterator of file entries to group
///
/// # Returns
///
/// A tuple of:
/// - `Vec<SizeGroup>` - Size groups with 2+ files, sorted by size descending
/// - `GroupingStats` - Statistics about the grouping operation
///
/// # Example
///
/// ```
/// use rustdupe::scanner::FileEntry;
/// use rustdupe::duplicates::group_by_size_structured;
/// use std::path::PathBuf;
/// use std::time::SystemTime;
///
/// let files = vec![
///     FileEntry::new(PathBuf::from("/a.txt"), 100, SystemTime::now()),
///     FileEntry::new(PathBuf::from("/b.txt"), 100, SystemTime::now()),
/// ];
///
/// let (groups, stats) = group_by_size_structured(files);
///
/// assert_eq!(groups.len(), 1);
/// assert_eq!(groups[0].size, 100);
/// assert!(groups[0].has_duplicates());
/// ```
#[must_use]
pub fn group_by_size_structured(
    files: impl IntoIterator<Item = FileEntry>,
) -> (Vec<SizeGroup>, GroupingStats) {
    let (groups_map, stats) = group_by_size(files);

    // Convert to SizeGroup structs and sort by size (largest first)
    let mut groups: Vec<SizeGroup> = groups_map
        .into_iter()
        .map(|(size, files)| SizeGroup::with_files(size, files))
        .collect();

    // Sort by size descending (prioritize larger files for potential savings)
    groups.sort_by(|a, b| b.size.cmp(&a.size));

    (groups, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    fn make_file(path: &str, size: u64) -> FileEntry {
        FileEntry::new(PathBuf::from(path), size, SystemTime::now())
    }

    #[test]
    fn test_size_group_new() {
        let group = SizeGroup::new(1024);
        assert_eq!(group.size, 1024);
        assert!(group.is_empty());
        assert!(!group.has_duplicates());
    }

    #[test]
    fn test_size_group_with_files() {
        let files = vec![make_file("/a.txt", 1024), make_file("/b.txt", 1024)];
        let group = SizeGroup::with_files(1024, files);

        assert_eq!(group.size, 1024);
        assert_eq!(group.len(), 2);
        assert!(group.has_duplicates());
    }

    #[test]
    fn test_size_group_add() {
        let mut group = SizeGroup::new(100);
        group.add(make_file("/a.txt", 100));
        group.add(make_file("/b.txt", 100));

        assert_eq!(group.len(), 2);
        assert!(group.has_duplicates());
    }

    #[test]
    fn test_size_group_total_size() {
        let files = vec![
            make_file("/a.txt", 1024),
            make_file("/b.txt", 1024),
            make_file("/c.txt", 1024),
        ];
        let group = SizeGroup::with_files(1024, files);

        assert_eq!(group.total_size(), 3072);
    }

    #[test]
    fn test_size_group_potential_savings() {
        let files = vec![
            make_file("/a.txt", 1024),
            make_file("/b.txt", 1024),
            make_file("/c.txt", 1024),
        ];
        let group = SizeGroup::with_files(1024, files);

        // If we keep one copy, we save 2 * 1024 = 2048 bytes
        assert_eq!(group.potential_savings(), 2048);
    }

    #[test]
    fn test_size_group_single_file_no_savings() {
        let group = SizeGroup::with_files(1024, vec![make_file("/a.txt", 1024)]);
        assert_eq!(group.potential_savings(), 0);
        assert!(!group.has_duplicates());
    }

    #[test]
    fn test_duplicate_group_wasted_space() {
        let group = DuplicateGroup::new(
            [0u8; 32],
            1000,
            vec![
                make_file("/a.txt", 1000),
                make_file("/b.txt", 1000),
                make_file("/c.txt", 1000),
            ],
            Vec::new(),
        );

        assert_eq!(group.wasted_space(), 2000); // 2 * 1000
        assert_eq!(group.duplicate_count(), 2);
    }

    #[test]
    fn test_duplicate_group_single_file() {
        let group =
            DuplicateGroup::new([0u8; 32], 1000, vec![make_file("/a.txt", 1000)], Vec::new());

        assert_eq!(group.wasted_space(), 0);
        assert_eq!(group.duplicate_count(), 0);
    }

    #[test]
    fn test_group_by_size_empty_input() {
        let files: Vec<FileEntry> = vec![];
        let (groups, stats) = group_by_size(files);

        assert!(groups.is_empty());
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.unique_sizes, 0);
        assert_eq!(stats.potential_duplicates, 0);
    }

    #[test]
    fn test_group_by_size_all_unique() {
        let files = vec![
            make_file("/a.txt", 100),
            make_file("/b.txt", 200),
            make_file("/c.txt", 300),
        ];
        let (groups, stats) = group_by_size(files);

        // No duplicates possible - all different sizes
        assert!(groups.is_empty());
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.unique_sizes, 3);
        assert_eq!(stats.eliminated_unique, 3);
        assert_eq!(stats.potential_duplicates, 0);
    }

    #[test]
    fn test_group_by_size_with_duplicates() {
        let files = vec![
            make_file("/a.txt", 100),
            make_file("/b.txt", 100),
            make_file("/c.txt", 200),
        ];
        let (groups, stats) = group_by_size(files);

        // Only the 100-byte group should remain
        assert_eq!(groups.len(), 1);
        assert!(groups.contains_key(&100));
        assert_eq!(groups[&100].len(), 2);

        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.unique_sizes, 2);
        assert_eq!(stats.eliminated_unique, 1); // The 200-byte file
        assert_eq!(stats.potential_duplicates, 2);
        assert_eq!(stats.duplicate_groups, 1);
    }

    #[test]
    fn test_group_by_size_multiple_groups() {
        let files = vec![
            make_file("/a1.txt", 100),
            make_file("/a2.txt", 100),
            make_file("/b1.txt", 200),
            make_file("/b2.txt", 200),
            make_file("/b3.txt", 200),
            make_file("/c.txt", 300), // unique
        ];
        let (groups, stats) = group_by_size(files);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[&100].len(), 2);
        assert_eq!(groups[&200].len(), 3);

        assert_eq!(stats.total_files, 6);
        assert_eq!(stats.unique_sizes, 3);
        assert_eq!(stats.eliminated_unique, 1);
        assert_eq!(stats.potential_duplicates, 5);
        assert_eq!(stats.duplicate_groups, 2);
    }

    #[test]
    fn test_group_by_size_empty_files_skipped() {
        let files = vec![
            make_file("/empty1.txt", 0),
            make_file("/empty2.txt", 0),
            make_file("/normal.txt", 100),
        ];
        let (groups, stats) = group_by_size(files);

        // Empty files should be skipped, only the unique 100-byte file remains
        // but it's eliminated as unique too
        assert!(groups.is_empty());
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.empty_files, 2);
        assert_eq!(stats.eliminated_unique, 1);
    }

    #[test]
    fn test_group_by_size_elimination_rate() {
        let files = vec![
            make_file("/a.txt", 100),
            make_file("/b.txt", 100),
            make_file("/c.txt", 200),
            make_file("/d.txt", 300),
        ];
        let (_, stats) = group_by_size(files);

        // 2 unique files eliminated out of 4 total = 50%
        assert!((stats.elimination_rate() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_group_by_size_structured() {
        let files = vec![
            make_file("/small1.txt", 100),
            make_file("/small2.txt", 100),
            make_file("/large1.txt", 10000),
            make_file("/large2.txt", 10000),
        ];
        let (groups, stats) = group_by_size_structured(files);

        // Should be sorted by size descending
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].size, 10000); // Largest first
        assert_eq!(groups[1].size, 100);

        assert_eq!(stats.total_files, 4);
        assert_eq!(stats.potential_duplicates, 4);
    }

    #[test]
    fn test_group_by_size_total_size_calculation() {
        let files = vec![
            make_file("/a.txt", 100),
            make_file("/b.txt", 200),
            make_file("/c.txt", 300),
        ];
        let (_, stats) = group_by_size(files);

        assert_eq!(stats.total_size, 600);
    }

    #[test]
    fn test_grouping_stats_default() {
        let stats = GroupingStats::default();

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.unique_sizes, 0);
        assert_eq!(stats.potential_duplicates, 0);
        assert_eq!(stats.eliminated_unique, 0);
        assert_eq!(stats.empty_files, 0);
        assert_eq!(stats.duplicate_groups, 0);
    }

    #[test]
    fn test_grouping_stats_elimination_rate_empty() {
        let stats = GroupingStats::default();
        assert_eq!(stats.elimination_rate(), 0.0);
    }

    #[test]
    fn test_duplicate_group_hash_hex() {
        let mut hash = [0u8; 32];
        hash[0] = 0xAB;
        hash[1] = 0xCD;
        hash[31] = 0xEF;

        let group = DuplicateGroup::new(hash, 100, vec![make_file("/a.txt", 100)], Vec::new());
        let hex = group.hash_hex();

        assert!(hex.starts_with("abcd"));
        assert!(hex.ends_with("ef"));
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn test_is_in_reference_dir() {
        let ref_paths = vec![
            PathBuf::from("/ref/path"),
            PathBuf::from("/other/ref"),
            PathBuf::from("/exact/match"),
        ];
        let group = DuplicateGroup::new([0u8; 32], 100, Vec::new(), ref_paths);

        // Subdirectory match
        assert!(group.is_in_reference_dir(Path::new("/ref/path/file.txt")));
        assert!(group.is_in_reference_dir(Path::new("/other/ref/sub/file.txt")));

        // Exact match
        assert!(group.is_in_reference_dir(Path::new("/exact/match")));

        // Non-match
        assert!(!group.is_in_reference_dir(Path::new("/normal/path/file.txt")));
        assert!(!group.is_in_reference_dir(Path::new("/ref/path_suffix/file.txt")));
        assert!(!group.is_in_reference_dir(Path::new("/ref/pat")));

        if cfg!(windows) {
            // Case-insensitive on Windows
            assert!(group.is_in_reference_dir(Path::new("/REF/PATH/file.txt")));
            assert!(group.is_in_reference_dir(Path::new("/Exact/Match")));
        } else {
            // Case-sensitive on non-Windows
            assert!(!group.is_in_reference_dir(Path::new("/REF/PATH/file.txt")));
        }
    }

    #[test]
    fn test_large_file_count_performance() {
        // Test that grouping 100,000 files is fast (metadata only, no I/O)
        use std::time::Instant;

        let files: Vec<FileEntry> = (0..100_000)
            .map(|i| {
                // Create groups: roughly 50% unique, 50% duplicates
                let size = if i % 2 == 0 {
                    i as u64
                } else {
                    (i / 100) as u64
                };
                make_file(&format!("/file{}.txt", i), size)
            })
            .collect();

        let start = Instant::now();
        let (groups, stats) = group_by_size(files);
        let elapsed = start.elapsed();

        assert_eq!(stats.total_files, 100_000);
        assert!(!groups.is_empty());

        // Should complete in under 1 second
        assert!(
            elapsed.as_secs() < 1,
            "Grouping took too long: {:?}",
            elapsed
        );
    }
}
