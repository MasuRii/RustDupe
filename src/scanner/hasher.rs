//! BLAKE3 file hasher with streaming support.
//!
//! # Overview
//!
//! This module provides the [`Hasher`] struct for computing BLAKE3 hashes
//! of file contents using memory-efficient streaming. It supports both
//! prehash (first N bytes) and full-file hashing operations.
//!
//! # Performance
//!
//! BLAKE3 is optimized for modern hardware:
//! - Single-threaded: ~8.4 GB/s on modern CPUs
//! - Multi-threaded: ~92 GB/s on 16 cores (with rayon feature)
//! - Uses 64KB buffer for optimal I/O throughput
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::scanner::hasher::Hasher;
//! use std::path::Path;
//!
//! let hasher = Hasher::new();
//!
//! // Compute prehash (first 4KB)
//! let prehash = hasher.prehash(Path::new("large_file.bin")).unwrap();
//!
//! // Compute full hash
//! let full_hash = hasher.full_hash(Path::new("large_file.bin")).unwrap();
//!
//! // For small files, prehash equals full hash
//! let small_hash = hasher.prehash(Path::new("small_file.txt")).unwrap();
//! ```

use std::fs::File;
use std::io::{BufReader, ErrorKind, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::HashError;

/// Default size for prehash - first 4KB of the file.
/// This is enough to detect most different files while minimizing I/O.
pub const PREHASH_SIZE: usize = 4 * 1024; // 4KB

/// Buffer size for streaming hash computation.
/// 64KB is optimal for modern SSD I/O and CPU cache efficiency.
const BUFFER_SIZE: usize = 64 * 1024; // 64KB

/// BLAKE3 hash output size (32 bytes / 256 bits).
pub type Hash = [u8; 32];

/// File hasher using BLAKE3 algorithm with streaming support.
///
/// The hasher is stateless and can be shared across threads.
/// It uses streaming to avoid loading entire files into memory.
///
/// # Thread Safety
///
/// `Hasher` is `Send + Sync` and can be safely shared across threads.
/// Each hashing operation uses its own temporary state.
#[derive(Debug, Clone)]
pub struct Hasher {
    /// Size of data to read for prehash operations
    prehash_size: usize,
    /// Optional shutdown flag for graceful termination
    shutdown_flag: Option<Arc<AtomicBool>>,
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher {
    /// Create a new hasher with default settings.
    ///
    /// Uses 4KB prehash size and 64KB buffer for streaming.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hasher::Hasher;
    /// let hasher = Hasher::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            prehash_size: PREHASH_SIZE,
            shutdown_flag: None,
        }
    }

    /// Create a hasher with custom prehash size.
    ///
    /// # Arguments
    ///
    /// * `prehash_size` - Number of bytes to read for prehash operations
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hasher::Hasher;
    /// let hasher = Hasher::with_prehash_size(8 * 1024); // 8KB
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `prehash_size` is 0.
    #[must_use]
    pub fn with_prehash_size(prehash_size: usize) -> Self {
        assert!(prehash_size > 0, "prehash_size must be greater than 0");
        Self {
            prehash_size,
            shutdown_flag: None,
        }
    }

    /// Set the shutdown flag for graceful termination.
    ///
    /// When the flag is set to `true`, long-running hash operations
    /// will abort and return an error.
    ///
    /// # Arguments
    ///
    /// * `flag` - Atomic boolean flag shared across threads
    /// # Example
    ///
    /// ```
    /// use rustdupe::scanner::hasher::Hasher;
    /// use std::sync::Arc;
    /// use std::sync::atomic::AtomicBool;
    ///
    /// let shutdown = Arc::new(AtomicBool::new(false));
    /// let hasher = Hasher::new().with_shutdown_flag(shutdown);
    /// ```
    #[must_use]
    pub fn with_shutdown_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.shutdown_flag = Some(flag);
        self
    }

    /// Check if shutdown has been requested.
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }

    /// Compute hash of the first N bytes of a file (prehash).
    ///
    /// Prehash is used in Phase 2 of duplicate detection to quickly
    /// eliminate files that differ in their first bytes.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to hash
    ///
    /// # Returns
    ///
    /// - `Ok(Hash)` - 32-byte BLAKE3 hash of the first N bytes
    /// - `Err(HashError)` - If the file cannot be read
    ///
    /// # Notes
    ///
    /// - For files smaller than the prehash size, the entire file is hashed
    /// - The resulting hash will equal `full_hash()` for small files
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::scanner::hasher::Hasher;
    /// use std::path::Path;
    ///
    /// let hasher = Hasher::new();
    /// let hash = hasher.prehash(Path::new("file.txt")).unwrap();
    /// println!("Prehash: {:x?}", hash);
    /// ```
    pub fn prehash(&self, path: &Path) -> Result<Hash, HashError> {
        self.hash_bytes(path, Some(self.prehash_size))
    }

    /// Compute hash of the entire file content.
    ///
    /// Uses streaming to avoid loading the entire file into memory.
    /// Suitable for files of any size, including those >4GB.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to hash
    ///
    /// # Returns
    ///
    /// - `Ok(Hash)` - 32-byte BLAKE3 hash of the entire file
    /// - `Err(HashError)` - If the file cannot be read
    ///
    /// # Performance
    ///
    /// - Uses 64KB buffer for optimal I/O
    /// - Supports graceful shutdown via shutdown flag
    /// - Typical speed: 1GB in <2 seconds on SSD
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::scanner::hasher::Hasher;
    /// use std::path::Path;
    ///
    /// let hasher = Hasher::new();
    /// let hash = hasher.full_hash(Path::new("large_file.bin")).unwrap();
    /// println!("Full hash: {:x?}", hash);
    /// ```
    pub fn full_hash(&self, path: &Path) -> Result<Hash, HashError> {
        self.hash_bytes(path, None)
    }

    /// Internal method to hash a file with optional byte limit.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    /// * `max_bytes` - Optional limit on bytes to read (None = entire file)
    fn hash_bytes(&self, path: &Path, max_bytes: Option<usize>) -> Result<Hash, HashError> {
        // Open the file
        let file = File::open(path).map_err(|e| self.map_io_error(path, e))?;

        // Use buffered reader for better I/O performance
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);

        // Create BLAKE3 hasher
        let mut hasher = blake3::Hasher::new();

        // Read and hash in chunks
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut total_read: u64 = 0;
        let limit = max_bytes.map(|b| b as u64);

        loop {
            // Check shutdown flag periodically (every buffer read)
            if self.is_shutdown_requested() {
                log::debug!("Hash operation interrupted for: {}", path.display());
                return Err(HashError::Io {
                    path: path.to_path_buf(),
                    source: std::io::Error::new(ErrorKind::Interrupted, "Operation interrupted"),
                });
            }

            // Determine how many bytes to read this iteration
            let bytes_to_read = if let Some(max) = limit {
                let remaining = max.saturating_sub(total_read);
                if remaining == 0 {
                    break;
                }
                buffer.len().min(remaining as usize)
            } else {
                buffer.len()
            };

            // Read from file
            let bytes_read = reader
                .read(&mut buffer[..bytes_to_read])
                .map_err(|e| self.map_io_error(path, e))?;

            if bytes_read == 0 {
                break; // EOF
            }

            // Update hash with read bytes
            hasher.update(&buffer[..bytes_read]);
            total_read += bytes_read as u64;

            // Check if we've read enough for limited hash
            if let Some(max) = limit {
                if total_read >= max {
                    break;
                }
            }
        }

        // Finalize and return hash
        Ok(*hasher.finalize().as_bytes())
    }

    /// Map I/O error to HashError with appropriate type.
    fn map_io_error(&self, path: &Path, error: std::io::Error) -> HashError {
        match error.kind() {
            ErrorKind::NotFound => {
                log::debug!("File not found (TOCTOU): {}", path.display());
                HashError::NotFound(path.to_path_buf())
            }
            ErrorKind::PermissionDenied => {
                log::warn!("Permission denied: {}", path.display());
                HashError::PermissionDenied(path.to_path_buf())
            }
            _ => {
                log::warn!("I/O error for {}: {}", path.display(), error);
                HashError::Io {
                    path: path.to_path_buf(),
                    source: error,
                }
            }
        }
    }

    /// Compute hash using the optimized update_reader method.
    ///
    /// This is an alternative implementation that uses BLAKE3's
    /// built-in streaming support. May be faster for large files.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to hash
    ///
    /// # Returns
    ///
    /// - `Ok(Hash)` - 32-byte BLAKE3 hash of the entire file
    /// - `Err(HashError)` - If the file cannot be read
    ///
    /// # Notes
    ///
    /// This method does NOT support the shutdown flag since it
    /// delegates entirely to BLAKE3's internal reader. Use `full_hash()`
    /// if you need interruptible hashing.
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::scanner::hasher::Hasher;
    /// use std::path::Path;
    ///
    /// let hasher = Hasher::new();
    /// let hash = hasher.full_hash_optimized(Path::new("large_file.bin")).unwrap();
    /// ```
    pub fn full_hash_optimized(&self, path: &Path) -> Result<Hash, HashError> {
        let file = File::open(path).map_err(|e| self.map_io_error(path, e))?;
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);

        let mut hasher = blake3::Hasher::new();

        // Use BLAKE3's optimized update_reader
        hasher
            .update_reader(&mut reader)
            .map_err(|e| self.map_io_error(path, e))?;

        Ok(*hasher.finalize().as_bytes())
    }
}

/// Format a hash as a hex string for display.
///
/// # Example
///
/// ```
/// use rustdupe::scanner::hasher::hash_to_hex;
///
/// let hash = [0u8; 32];
/// let hex = hash_to_hex(&hash);
/// assert_eq!(hex.len(), 64);
/// ```
#[must_use]
pub fn hash_to_hex(hash: &Hash) -> String {
    use std::fmt::Write;
    hash.iter().fold(String::with_capacity(64), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// Parse a hex string to a hash.
///
/// # Arguments
///
/// * `hex` - 64-character hex string
///
/// # Returns
///
/// - `Some(Hash)` - If parsing succeeded
/// - `None` - If the string is invalid
///
/// # Example
///
/// ```
/// use rustdupe::scanner::hasher::{hash_to_hex, hex_to_hash};
///
/// let original = [1u8; 32];
/// let hex = hash_to_hex(&original);
/// let parsed = hex_to_hash(&hex).unwrap();
/// assert_eq!(original, parsed);
/// ```
pub fn hex_to_hash(hex: &str) -> Option<Hash> {
    if hex.len() != 64 {
        return None;
    }

    let mut hash = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let byte_str = std::str::from_utf8(chunk).ok()?;
        hash[i] = u8::from_str_radix(byte_str, 16).ok()?;
    }
    Some(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut file = File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

    #[test]
    fn test_hasher_identical_content_same_hash() {
        let dir = TempDir::new().unwrap();
        let content = b"Hello, world!";

        let file1 = create_test_file(&dir, "file1.txt", content);
        let file2 = create_test_file(&dir, "file2.txt", content);

        let hasher = Hasher::new();
        let hash1 = hasher.full_hash(&file1).unwrap();
        let hash2 = hasher.full_hash(&file2).unwrap();

        assert_eq!(
            hash1, hash2,
            "Identical content should produce identical hashes"
        );
    }

    #[test]
    fn test_hasher_different_content_different_hash() {
        let dir = TempDir::new().unwrap();

        let file1 = create_test_file(&dir, "file1.txt", b"Hello");
        let file2 = create_test_file(&dir, "file2.txt", b"World");

        let hasher = Hasher::new();
        let hash1 = hasher.full_hash(&file1).unwrap();
        let hash2 = hasher.full_hash(&file2).unwrap();

        assert_ne!(
            hash1, hash2,
            "Different content should produce different hashes"
        );
    }

    #[test]
    fn test_prehash_small_file_equals_full_hash() {
        let dir = TempDir::new().unwrap();
        let content = b"Small file content"; // < 4KB

        let file = create_test_file(&dir, "small.txt", content);

        let hasher = Hasher::new();
        let prehash = hasher.prehash(&file).unwrap();
        let full_hash = hasher.full_hash(&file).unwrap();

        assert_eq!(
            prehash, full_hash,
            "Prehash of small file should equal full hash"
        );
    }

    #[test]
    fn test_prehash_large_file_differs_from_full_hash() {
        let dir = TempDir::new().unwrap();

        // Create a file larger than 4KB with varying content
        let mut content = vec![0u8; 8 * 1024]; // 8KB
        for (i, byte) in content.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        let file = create_test_file(&dir, "large.bin", &content);

        let hasher = Hasher::new();
        let prehash = hasher.prehash(&file).unwrap();
        let full_hash = hasher.full_hash(&file).unwrap();

        assert_ne!(
            prehash, full_hash,
            "Prehash of large file should differ from full hash"
        );
    }

    #[test]
    fn test_empty_file_hash() {
        let dir = TempDir::new().unwrap();
        let file = create_test_file(&dir, "empty.txt", b"");

        let hasher = Hasher::new();
        let hash = hasher.full_hash(&file).unwrap();

        // BLAKE3 empty hash is known
        let expected_empty_hash = blake3::hash(b"");
        assert_eq!(hash, *expected_empty_hash.as_bytes());
    }

    #[test]
    fn test_hash_deterministic() {
        let dir = TempDir::new().unwrap();
        let content = b"Deterministic content";
        let file = create_test_file(&dir, "det.txt", content);

        let hasher = Hasher::new();

        // Hash the same file multiple times
        let hash1 = hasher.full_hash(&file).unwrap();
        let hash2 = hasher.full_hash(&file).unwrap();
        let hash3 = hasher.full_hash(&file).unwrap();

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_file_not_found_error() {
        let hasher = Hasher::new();
        let result = hasher.full_hash(Path::new("/nonexistent/file/12345.txt"));

        assert!(result.is_err());
        match result.unwrap_err() {
            HashError::NotFound(path) => {
                assert!(path.to_string_lossy().contains("12345.txt"));
            }
            other => panic!("Expected NotFound error, got: {:?}", other),
        }
    }

    #[test]
    fn test_full_hash_optimized_matches_regular() {
        let dir = TempDir::new().unwrap();
        let content = b"Test content for optimized hash";
        let file = create_test_file(&dir, "opt.txt", content);

        let hasher = Hasher::new();
        let regular = hasher.full_hash(&file).unwrap();
        let optimized = hasher.full_hash_optimized(&file).unwrap();

        assert_eq!(regular, optimized);
    }

    #[test]
    fn test_hash_to_hex() {
        let hash = [0xAB; 32];
        let hex = hash_to_hex(&hash);

        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hex, "ab".repeat(32));
    }

    #[test]
    fn test_hex_to_hash_roundtrip() {
        let original: Hash = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
            0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67,
            0x89, 0xAB, 0xCD, 0xEF,
        ];

        let hex = hash_to_hex(&original);
        let parsed = hex_to_hash(&hex).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_hex_to_hash_invalid() {
        assert!(hex_to_hash("").is_none());
        assert!(hex_to_hash("abc").is_none());
        assert!(hex_to_hash("gg".repeat(32).as_str()).is_none());
    }

    #[test]
    fn test_custom_prehash_size() {
        let dir = TempDir::new().unwrap();

        // Create file with known content
        let mut content = vec![0u8; 2048];
        for (i, byte) in content.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        let file = create_test_file(&dir, "custom.bin", &content);

        let hasher_1k = Hasher::with_prehash_size(1024);
        let hasher_2k = Hasher::with_prehash_size(2048);

        let hash_1k = hasher_1k.prehash(&file).unwrap();
        let hash_2k = hasher_2k.prehash(&file).unwrap();

        // Different prehash sizes should produce different hashes
        assert_ne!(hash_1k, hash_2k);

        // 2KB prehash of 2KB file should equal full hash
        let full = Hasher::new().full_hash(&file).unwrap();
        assert_eq!(hash_2k, full);
    }

    #[test]
    fn test_shutdown_flag_interrupts_hash() {
        let dir = TempDir::new().unwrap();

        // Create a file large enough to have multiple read iterations
        let content = vec![0u8; 256 * 1024]; // 256KB
        let file = create_test_file(&dir, "large.bin", &content);

        let shutdown = Arc::new(AtomicBool::new(true)); // Already set
        let hasher = Hasher::new().with_shutdown_flag(shutdown);

        let result = hasher.full_hash(&file);

        assert!(result.is_err());
        match result.unwrap_err() {
            HashError::Io { source, .. } => {
                assert_eq!(source.kind(), ErrorKind::Interrupted);
            }
            other => panic!("Expected Io error with Interrupted, got: {:?}", other),
        }
    }

    #[test]
    fn test_hasher_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Hasher>();
    }

    #[test]
    #[should_panic(expected = "prehash_size must be greater than 0")]
    fn test_zero_prehash_size_panics() {
        let _ = Hasher::with_prehash_size(0);
    }
}
