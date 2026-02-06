//! Cache entry definitions.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::scanner::{FileEntry, Hash, ImageHash};

/// Represents a single file entry in the hash cache.
///
/// A cache entry stores metadata and computed hashes for a file.
/// It is used to avoid re-hashing files that haven't changed.
///
/// Invalidation is handled by comparing the stored `size` and `mtime`
/// with the current file on disk. `inode` is used for additional
/// verification on supported platforms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheEntry {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub mtime: SystemTime,
    /// Optional file inode (if available on the platform).
    pub inode: Option<u64>,
    /// Prehash of the file (first N bytes).
    pub prehash: Hash,
    /// Optional full hash of the file.
    pub fullhash: Option<Hash>,
    /// Optional perceptual hash of the image.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::scanner::perceptual_hash_serde"
    )]
    pub perceptual_hash: Option<ImageHash>,
    /// Optional document fingerprint for similarity detection (SimHash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_fingerprint: Option<u64>,
}

impl CacheEntry {
    /// Generate a unique key for the cache entry based on file metadata.
    ///
    /// The key consists of path, size, modification time, and optionally inode.
    /// If any of these change, the cache entry is considered invalid for that file.
    #[must_use]
    pub fn generate_key(path: &Path, size: u64, mtime: SystemTime, inode: Option<u64>) -> String {
        format!(
            "{}:{}:{}:{}",
            path.to_string_lossy(),
            size,
            mtime
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            inode.unwrap_or(0)
        )
    }

    /// Check if this cache entry is still valid for a given file metadata.
    #[must_use]
    pub fn is_valid(&self, size: u64, mtime: SystemTime, inode: Option<u64>) -> bool {
        if self.size != size || self.mtime != mtime {
            return false;
        }

        // If both have inodes, they must match.
        // If either is None, we skip inode validation.
        match (self.inode, inode) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
}

impl From<FileEntry> for CacheEntry {
    /// Create a new `CacheEntry` from a `FileEntry`.
    ///
    /// Note: `prehash` is initialized to zeros and `fullhash` to `None`.
    /// These must be updated after hashing.
    fn from(entry: FileEntry) -> Self {
        Self {
            path: entry.path,
            size: entry.size,
            mtime: entry.modified,
            inode: None, // FileEntry currently doesn't store inode
            prehash: [0u8; 32],
            fullhash: None,
            perceptual_hash: entry.perceptual_hash,
            document_fingerprint: entry.document_fingerprint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_cache_entry_is_valid() {
        let now = SystemTime::now();
        let entry = CacheEntry {
            path: PathBuf::from("/test.txt"),
            size: 100,
            mtime: now,
            inode: Some(123),
            prehash: [0u8; 32],
            fullhash: None,
            perceptual_hash: None,
            document_fingerprint: None,
        };

        assert!(entry.is_valid(100, now, Some(123)));
        assert!(entry.is_valid(100, now, None)); // Inode None in check still valid if stored inode matches or if we don't care
        assert!(!entry.is_valid(101, now, Some(123)));
        assert!(!entry.is_valid(100, now + Duration::from_secs(1), Some(123)));
        assert!(!entry.is_valid(100, now, Some(456)));
    }

    #[test]
    fn test_generate_key() {
        let now = SystemTime::now();
        let key1 = CacheEntry::generate_key(Path::new("/test.txt"), 100, now, Some(123));
        let key2 = CacheEntry::generate_key(Path::new("/test.txt"), 100, now, Some(123));
        let key3 = CacheEntry::generate_key(Path::new("/test.txt"), 101, now, Some(123));

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }
}
