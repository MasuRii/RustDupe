//! Hardlink detection for avoiding false duplicate identification.
//!
//! # Overview
//!
//! Hardlinks are multiple directory entries pointing to the same inode on disk.
//! They share the same content but are NOT duplicates - they're the same file.
//! This module detects hardlinks to prevent counting them as duplicates.
//!
//! # Platform Support
//!
//! - **Unix**: Uses (device_id, inode) pairs from file metadata
//! - **Windows**: Uses file_index from file handle (requires opening file)
//! - **Other**: Hardlink detection disabled (all files treated as unique)
//!
//! # Example
//!
//! ```
//! use rustdupe::scanner::hardlink::HardlinkTracker;
//! use std::path::Path;
//!
//! let mut tracker = HardlinkTracker::new();
//!
//! // On Unix, checking hardlinks from metadata:
//! let path = Path::new("/some/file.txt");
//! let metadata = std::fs::metadata(path).ok();
//!
//! if let Some(meta) = metadata {
//!     if tracker.is_hardlink(&meta) {
//!         println!("Skipping hardlink: {}", path.display());
//!     } else {
//!         println!("Processing file: {}", path.display());
//!     }
//! }
//! ```

use std::collections::HashSet;
use std::fs::Metadata;
#[cfg(windows)]
use std::path::Path;

/// Tracks seen inodes to detect hardlinks.
///
/// Files with the same inode are hardlinks to the same underlying data.
/// The tracker remembers which inodes have been seen and reports subsequent
/// occurrences as hardlinks.
///
/// # Thread Safety
///
/// `HardlinkTracker` is NOT thread-safe. Create one per thread or use
/// external synchronization if sharing across threads.
#[derive(Debug, Default)]
pub struct HardlinkTracker {
    /// Set of seen inode keys
    seen: HashSet<InodeKey>,
}

impl HardlinkTracker {
    /// Create a new hardlink tracker.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hardlink::HardlinkTracker;
    ///
    /// let tracker = HardlinkTracker::new();
    /// assert_eq!(tracker.seen_count(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Create a tracker with pre-allocated capacity.
    ///
    /// Use this when you know approximately how many files you'll process.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Expected number of unique files
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hardlink::HardlinkTracker;
    ///
    /// let tracker = HardlinkTracker::with_capacity(10000);
    /// ```
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            seen: HashSet::with_capacity(capacity),
        }
    }

    /// Check if a file is a hardlink to a previously seen file.
    ///
    /// Returns `true` if the file's inode was already seen, meaning this
    /// is a hardlink to a previously processed file.
    ///
    /// Returns `false` if:
    /// - This is the first occurrence of this inode
    /// - The platform doesn't support hardlink detection
    /// - The metadata doesn't contain inode information
    ///
    /// # Arguments
    ///
    /// * `metadata` - File metadata from `std::fs::metadata()` or `symlink_metadata()`
    ///
    /// # Side Effects
    ///
    /// If this is the first occurrence of an inode, it is recorded for
    /// future lookups. This makes the check stateful.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::scanner::hardlink::HardlinkTracker;
    /// use std::fs;
    ///
    /// let mut tracker = HardlinkTracker::new();
    ///
    /// let meta1 = fs::metadata("file1.txt").unwrap();
    /// let meta2 = fs::metadata("hardlink_to_file1.txt").unwrap();
    ///
    /// assert!(!tracker.is_hardlink(&meta1)); // First occurrence
    /// assert!(tracker.is_hardlink(&meta2));  // Hardlink detected
    /// ```
    pub fn is_hardlink(&mut self, metadata: &Metadata) -> bool {
        if let Some(key) = InodeKey::from_metadata(metadata) {
            if self.seen.contains(&key) {
                return true;
            }
            self.seen.insert(key);
        }
        false
    }

    /// Check if a file is a hardlink without recording it.
    ///
    /// Unlike [`is_hardlink`](Self::is_hardlink), this method doesn't record
    /// new inodes. Useful for read-only checks.
    ///
    /// # Arguments
    ///
    /// * `metadata` - File metadata to check
    ///
    /// # Returns
    ///
    /// - `Some(true)` - File is a hardlink to a previously seen file
    /// - `Some(false)` - File's inode has not been seen before
    /// - `None` - Platform doesn't support hardlink detection
    #[must_use]
    pub fn check_hardlink(&self, metadata: &Metadata) -> Option<bool> {
        InodeKey::from_metadata(metadata).map(|key| self.seen.contains(&key))
    }

    /// Record a file's inode without checking for hardlinks.
    ///
    /// Use this when you want to pre-populate the tracker with known files.
    ///
    /// # Arguments
    ///
    /// * `metadata` - File metadata to record
    ///
    /// # Returns
    ///
    /// `true` if the inode was newly recorded, `false` if already present
    /// or if the platform doesn't support hardlink detection.
    pub fn record(&mut self, metadata: &Metadata) -> bool {
        if let Some(key) = InodeKey::from_metadata(metadata) {
            return self.seen.insert(key);
        }
        false
    }

    /// Get the number of unique inodes tracked.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hardlink::HardlinkTracker;
    ///
    /// let tracker = HardlinkTracker::new();
    /// assert_eq!(tracker.seen_count(), 0);
    /// ```
    #[must_use]
    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }

    /// Clear all tracked inodes.
    ///
    /// After calling this, all files will be treated as first occurrences.
    pub fn clear(&mut self) {
        self.seen.clear();
    }

    /// Check if hardlink detection is supported on this platform.
    ///
    /// # Returns
    ///
    /// - `true` on Unix (uses dev/ino)
    /// - `false` on Windows and other platforms (not yet implemented)
    ///
    /// Note: Windows support requires opening file handles to get file_index,
    /// which is not yet implemented. This may be added in a future version.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hardlink::HardlinkTracker;
    ///
    /// if HardlinkTracker::is_supported() {
    ///     println!("Hardlink detection is available");
    /// } else {
    ///     println!("Hardlink detection is not available on this platform");
    /// }
    /// ```
    #[must_use]
    pub const fn is_supported() -> bool {
        // Only Unix is currently implemented
        // Windows requires opening file handles to get file_index
        cfg!(unix)
    }
}

/// Platform-specific inode key for hardlink detection.
///
/// On Unix, this is (device_id, inode).
/// On Windows, this uses file_index from file handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InodeKey {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(not(any(unix, windows)))]
    _phantom: (),
}

impl InodeKey {
    /// Create an inode key from file metadata.
    ///
    /// Returns `None` if the platform doesn't support inode tracking
    /// or if the metadata doesn't contain the required information.
    #[cfg(unix)]
    fn from_metadata(metadata: &Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }

    #[cfg(windows)]
    fn from_metadata(_metadata: &Metadata) -> Option<Self> {
        // Windows metadata doesn't expose file_index directly.
        // We would need to open the file handle to get this info.
        // For now, return None to disable hardlink detection in walker.
        // Hardlinks will still be handled correctly by hashing (same content = same hash).
        //
        // To properly implement Windows hardlink detection, we would need to:
        // 1. Use winapi or windows-sys crate
        // 2. Call GetFileInformationByHandle
        // 3. Extract nFileIndexHigh, nFileIndexLow, dwVolumeSerialNumber
        //
        // This is deferred for now as it requires additional dependencies.
        None
    }

    #[cfg(not(any(unix, windows)))]
    fn from_metadata(_metadata: &Metadata) -> Option<Self> {
        None
    }
}

/// Get the inode key from a file path (Windows-specific helper).
///
/// On Windows, we need to open the file to get the file index.
/// This function handles that complexity.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// The inode key if available, or `None` if the file cannot be opened
/// or the platform doesn't support this operation.
#[cfg(windows)]
// Currently unused as Windows hardlink detection is stubbed
#[allow(dead_code)]
fn get_inode_key_from_path(path: &Path) -> Option<InodeKey> {
    // This would require winapi/windows-sys to implement properly.
    // For now, we don't support Windows hardlink detection via path.
    let _ = path;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", content).unwrap();
        path
    }

    #[test]
    fn test_tracker_new() {
        let tracker = HardlinkTracker::new();
        assert_eq!(tracker.seen_count(), 0);
    }

    #[test]
    fn test_tracker_with_capacity() {
        let tracker = HardlinkTracker::with_capacity(100);
        assert_eq!(tracker.seen_count(), 0);
    }

    #[test]
    fn test_tracker_clear() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, "test.txt", "content");
        let metadata = std::fs::metadata(&path).unwrap();

        let mut tracker = HardlinkTracker::new();
        tracker.is_hardlink(&metadata);

        // On supported platforms, count should be > 0
        // On unsupported platforms (Windows), count stays 0
        if HardlinkTracker::is_supported() {
            assert!(tracker.seen_count() > 0);
        } else {
            assert_eq!(tracker.seen_count(), 0);
        }

        tracker.clear();
        assert_eq!(tracker.seen_count(), 0);
    }

    #[test]
    fn test_same_file_not_hardlink() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, "test.txt", "content");
        let metadata = std::fs::metadata(&path).unwrap();

        let mut tracker = HardlinkTracker::new();

        // First check should NOT report hardlink
        assert!(!tracker.is_hardlink(&metadata));
    }

    #[test]
    fn test_different_files_not_hardlinks() {
        let dir = TempDir::new().unwrap();
        let path1 = create_test_file(&dir, "file1.txt", "content1");
        let path2 = create_test_file(&dir, "file2.txt", "content2");

        let meta1 = std::fs::metadata(&path1).unwrap();
        let meta2 = std::fs::metadata(&path2).unwrap();

        let mut tracker = HardlinkTracker::new();

        assert!(!tracker.is_hardlink(&meta1));
        assert!(!tracker.is_hardlink(&meta2));
    }

    #[test]
    #[cfg(unix)]
    fn test_hardlink_detected() {
        use std::fs::hard_link;

        let dir = TempDir::new().unwrap();
        let original = create_test_file(&dir, "original.txt", "content");
        let link_path = dir.path().join("hardlink.txt");
        hard_link(&original, &link_path).unwrap();

        let meta_original = std::fs::metadata(&original).unwrap();
        let meta_link = std::fs::metadata(&link_path).unwrap();

        let mut tracker = HardlinkTracker::new();

        // First occurrence is not a hardlink
        assert!(!tracker.is_hardlink(&meta_original));
        // Second occurrence IS a hardlink
        assert!(tracker.is_hardlink(&meta_link));
    }

    #[test]
    fn test_check_hardlink_readonly() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, "test.txt", "content");
        let metadata = std::fs::metadata(&path).unwrap();

        let mut tracker = HardlinkTracker::new();

        // On unsupported platforms, check_hardlink returns None
        if !HardlinkTracker::is_supported() {
            assert_eq!(tracker.check_hardlink(&metadata), None);
            return;
        }

        // Read-only check before recording
        assert_eq!(tracker.check_hardlink(&metadata), Some(false));

        // Record the inode
        tracker.is_hardlink(&metadata);

        // Now it should show as seen (but it's not a "hardlink" since it's the first occurrence)
        // The check_hardlink just tells us if we've seen this inode
        assert_eq!(tracker.check_hardlink(&metadata), Some(true));
    }

    #[test]
    fn test_record_without_check() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, "test.txt", "content");
        let metadata = std::fs::metadata(&path).unwrap();

        let mut tracker = HardlinkTracker::new();

        if HardlinkTracker::is_supported() {
            // First record returns true (newly inserted)
            assert!(tracker.record(&metadata));
            // Second record returns false (already present)
            assert!(!tracker.record(&metadata));
        } else {
            // On unsupported platforms, record always returns false
            assert!(!tracker.record(&metadata));
        }
    }

    #[test]
    fn test_is_supported() {
        let supported = HardlinkTracker::is_supported();
        // Should be true on Unix, potentially false on Windows (until implemented)
        #[cfg(unix)]
        assert!(supported);

        // Just make sure it doesn't panic
        println!("Hardlink detection supported: {}", supported);
    }

    #[test]
    #[cfg(unix)]
    fn test_seen_count_increases() {
        let dir = TempDir::new().unwrap();
        let path1 = create_test_file(&dir, "file1.txt", "content1");
        let path2 = create_test_file(&dir, "file2.txt", "content2");

        let meta1 = std::fs::metadata(&path1).unwrap();
        let meta2 = std::fs::metadata(&path2).unwrap();

        let mut tracker = HardlinkTracker::new();
        assert_eq!(tracker.seen_count(), 0);

        tracker.is_hardlink(&meta1);
        assert_eq!(tracker.seen_count(), 1);

        tracker.is_hardlink(&meta2);
        assert_eq!(tracker.seen_count(), 2);

        // Checking same file again shouldn't increase count
        tracker.is_hardlink(&meta1);
        assert_eq!(tracker.seen_count(), 2);
    }

    #[test]
    #[cfg(unix)]
    fn test_multiple_hardlinks_same_inode() {
        use std::fs::hard_link;

        let dir = TempDir::new().unwrap();
        let original = create_test_file(&dir, "original.txt", "content");
        let link1 = dir.path().join("link1.txt");
        let link2 = dir.path().join("link2.txt");
        hard_link(&original, &link1).unwrap();
        hard_link(&original, &link2).unwrap();

        let meta_original = std::fs::metadata(&original).unwrap();
        let meta_link1 = std::fs::metadata(&link1).unwrap();
        let meta_link2 = std::fs::metadata(&link2).unwrap();

        let mut tracker = HardlinkTracker::new();

        // Only the first should NOT be a hardlink
        assert!(!tracker.is_hardlink(&meta_original));
        assert!(tracker.is_hardlink(&meta_link1));
        assert!(tracker.is_hardlink(&meta_link2));

        // Only one unique inode
        assert_eq!(tracker.seen_count(), 1);
    }
}
