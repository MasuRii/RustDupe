//! Duplicate finder implementation with multi-phase detection.
//!
//! # Overview
//!
//! This module orchestrates the duplicate detection pipeline:
//! 1. **Phase 1 - Size grouping**: Group files by size (see [`groups`] module)
//! 2. **Phase 2 - Prehash**: Hash first 4KB of same-size files
//! 3. **Phase 3 - Full hash**: Hash entire content of prehash matches
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::scanner::{Walker, WalkerConfig, FileEntry, Hasher};
//! use rustdupe::duplicates::{group_by_size, phase2_prehash, PrehashConfig};
//! use std::path::Path;
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicBool;
//!
//! // Phase 1: Collect and group files by size
//! let walker = Walker::new(Path::new("."), WalkerConfig::default());
//! let files: Vec<FileEntry> = walker.walk().filter_map(Result::ok).collect();
//! let (size_groups, size_stats) = group_by_size(files);
//!
//! // Phase 2: Compute prehashes for potential duplicates
//! let hasher = Arc::new(Hasher::new());
//! let config = PrehashConfig::default();
//! let (prehash_groups, prehash_stats) = phase2_prehash(size_groups, hasher, config);
//!
//! println!("Phase 2: {} potential duplicates remain", prehash_stats.potential_duplicates);
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::scanner::{FileEntry, Hash, Hasher};

/// Configuration for prehash phase.
#[derive(Clone)]
pub struct PrehashConfig {
    /// Number of I/O threads for parallel hashing.
    /// Default is 4 to prevent disk thrashing.
    pub io_threads: usize,
    /// Optional shutdown flag for graceful termination.
    pub shutdown_flag: Option<Arc<AtomicBool>>,
    /// Optional progress callback.
    pub progress_callback: Option<Arc<dyn ProgressCallback>>,
}

impl std::fmt::Debug for PrehashConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrehashConfig")
            .field("io_threads", &self.io_threads)
            .field("shutdown_flag", &self.shutdown_flag)
            .field(
                "progress_callback",
                &self.progress_callback.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

impl Default for PrehashConfig {
    fn default() -> Self {
        Self {
            io_threads: 4,
            shutdown_flag: None,
            progress_callback: None,
        }
    }
}

impl PrehashConfig {
    /// Create a new configuration with custom I/O thread count.
    #[must_use]
    pub fn with_io_threads(mut self, threads: usize) -> Self {
        self.io_threads = threads.max(1);
        self
    }

    /// Set the shutdown flag for graceful termination.
    #[must_use]
    pub fn with_shutdown_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.shutdown_flag = Some(flag);
        self
    }

    /// Set the progress callback.
    #[must_use]
    pub fn with_progress_callback(mut self, callback: Arc<dyn ProgressCallback>) -> Self {
        self.progress_callback = Some(callback);
        self
    }

    /// Check if shutdown has been requested.
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }
}

/// Progress callback for duplicate finding phases.
///
/// Implement this trait to receive progress updates during
/// the duplicate detection pipeline.
pub trait ProgressCallback: Send + Sync {
    /// Called when a phase starts.
    ///
    /// # Arguments
    ///
    /// * `phase` - Name of the phase (e.g., "prehash", "fullhash")
    /// * `total` - Total number of items to process
    fn on_phase_start(&self, phase: &str, total: usize);

    /// Called for each item processed.
    ///
    /// # Arguments
    ///
    /// * `current` - Current item number (1-based)
    /// * `path` - Path being processed
    fn on_progress(&self, current: usize, path: &str);

    /// Called when a phase completes.
    ///
    /// # Arguments
    ///
    /// * `phase` - Name of the phase
    fn on_phase_end(&self, phase: &str);
}

/// Statistics from prehash phase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrehashStats {
    /// Total files that entered Phase 2
    pub input_files: usize,
    /// Number of files successfully hashed
    pub hashed_files: usize,
    /// Number of files that failed to hash (I/O errors)
    pub failed_files: usize,
    /// Number of unique prehashes (eliminated)
    pub unique_prehashes: usize,
    /// Number of files that could still be duplicates
    pub potential_duplicates: usize,
    /// Number of prehash groups with 2+ files
    pub duplicate_groups: usize,
    /// Whether phase was interrupted by shutdown
    pub interrupted: bool,
}

impl PrehashStats {
    /// Percentage of files eliminated by prehash comparison.
    #[must_use]
    pub fn elimination_rate(&self) -> f64 {
        if self.input_files == 0 {
            0.0
        } else {
            let eliminated = self.input_files - self.potential_duplicates;
            (eliminated as f64 / self.input_files as f64) * 100.0
        }
    }
}

/// A file with its computed prehash.
#[derive(Debug, Clone)]
pub struct PrehashEntry {
    /// Original file entry
    pub file: FileEntry,
    /// Computed prehash (first 4KB)
    pub prehash: Hash,
}

/// Group files by prehash within size groups (Phase 2).
///
/// This is the second phase of duplicate detection. For each size group,
/// it computes the prehash (first 4KB) of each file and groups files
/// with matching prehashes. This typically eliminates 80-95% of remaining
/// files.
///
/// # Arguments
///
/// * `size_groups` - Files grouped by size from Phase 1
/// * `hasher` - The hasher to use for computing prehashes
/// * `config` - Configuration for the prehash phase
///
/// # Returns
///
/// A tuple of:
/// - `HashMap<Hash, Vec<FileEntry>>` - Files grouped by prehash (only groups with 2+ files)
/// - `PrehashStats` - Statistics about the prehash operation
///
/// # Performance
///
/// - Uses rayon for parallel I/O (limited to `io_threads` to prevent disk thrashing)
/// - Reads only first 4KB of each file (O(m) where m is file count)
/// - For files smaller than 4KB, reads entire content
///
/// # Example
///
/// ```no_run
/// use rustdupe::scanner::{FileEntry, Hasher};
/// use rustdupe::duplicates::{group_by_size, phase2_prehash, PrehashConfig};
/// use std::sync::Arc;
///
/// // Assume files are already grouped by size
/// let files: Vec<FileEntry> = vec![];
/// let (size_groups, _) = group_by_size(files);
///
/// let hasher = Arc::new(Hasher::new());
/// let config = PrehashConfig::default();
/// let (prehash_groups, stats) = phase2_prehash(size_groups, hasher, config);
///
/// println!("Phase 2: {:.1}% eliminated by prehash", stats.elimination_rate());
/// ```
#[must_use]
pub fn phase2_prehash(
    size_groups: HashMap<u64, Vec<FileEntry>>,
    hasher: Arc<Hasher>,
    config: PrehashConfig,
) -> (HashMap<Hash, Vec<FileEntry>>, PrehashStats) {
    // Count total input files
    let input_files: usize = size_groups.values().map(|v| v.len()).sum();
    let mut stats = PrehashStats {
        input_files,
        ..Default::default()
    };

    // Flatten all files from size groups
    let all_files: Vec<FileEntry> = size_groups.into_values().flatten().collect();

    if all_files.is_empty() {
        log::debug!("Phase 2: No files to process");
        return (HashMap::new(), stats);
    }

    // Notify progress callback
    if let Some(ref callback) = config.progress_callback {
        callback.on_phase_start("prehash", all_files.len());
    }

    log::info!("Phase 2: Computing prehashes for {} files", all_files.len());

    // Build a custom thread pool with limited parallelism for I/O
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.io_threads)
        .build()
        .unwrap_or_else(|_| {
            log::warn!(
                "Failed to create custom thread pool, using global pool with {} threads",
                rayon::current_num_threads()
            );
            rayon::ThreadPoolBuilder::new().build().unwrap()
        });

    // Compute prehashes in parallel with limited I/O parallelism
    let prehash_results: Vec<(FileEntry, Option<Hash>)> = pool.install(|| {
        all_files
            .into_par_iter()
            .enumerate()
            .map(|(idx, file)| {
                // Check shutdown flag
                if config.is_shutdown_requested() {
                    log::debug!("Phase 2: Shutdown requested, skipping remaining files");
                    return (file, None);
                }

                // Report progress
                if let Some(ref callback) = config.progress_callback {
                    callback.on_progress(idx + 1, file.path.to_string_lossy().as_ref());
                }

                // Compute prehash
                match hasher.prehash(&file.path) {
                    Ok(hash) => {
                        log::trace!("Prehash computed: {}", file.path.display());
                        (file, Some(hash))
                    }
                    Err(e) => {
                        log::warn!("Failed to prehash {}: {}", file.path.display(), e);
                        (file, None)
                    }
                }
            })
            .collect()
    });

    // Check if we were interrupted
    if config.is_shutdown_requested() {
        stats.interrupted = true;
        log::info!("Phase 2: Interrupted by shutdown signal");
    }

    // Group by prehash
    let mut prehash_groups: HashMap<Hash, Vec<FileEntry>> = HashMap::new();

    for (file, prehash_opt) in prehash_results {
        match prehash_opt {
            Some(prehash) => {
                stats.hashed_files += 1;
                prehash_groups.entry(prehash).or_default().push(file);
            }
            None => {
                stats.failed_files += 1;
            }
        }
    }

    // Filter to only groups with 2+ files (potential duplicates)
    let filtered_groups: HashMap<Hash, Vec<FileEntry>> = prehash_groups
        .into_iter()
        .filter(|(hash, files)| {
            if files.len() == 1 {
                stats.unique_prehashes += 1;
                log::trace!(
                    "Eliminated unique prehash {}: {}",
                    crate::scanner::hash_to_hex(hash),
                    files[0].path.display()
                );
                false
            } else {
                stats.potential_duplicates += files.len();
                stats.duplicate_groups += 1;
                log::debug!(
                    "Prehash group {}: {} potential duplicates",
                    crate::scanner::hash_to_hex(hash),
                    files.len()
                );
                true
            }
        })
        .collect();

    // Notify progress callback
    if let Some(ref callback) = config.progress_callback {
        callback.on_phase_end("prehash");
    }

    log::info!(
        "Phase 2 complete: {} files → {} potential duplicates ({:.1}% eliminated)",
        stats.input_files,
        stats.potential_duplicates,
        stats.elimination_rate()
    );

    (filtered_groups, stats)
}

/// Flatten size groups into a list of prehash entries.
///
/// This is a helper function that computes prehashes for all files
/// in the size groups and returns them as a flat list.
#[must_use]
pub fn compute_prehashes(
    size_groups: HashMap<u64, Vec<FileEntry>>,
    hasher: Arc<Hasher>,
    config: PrehashConfig,
) -> Vec<PrehashEntry> {
    let all_files: Vec<FileEntry> = size_groups.into_values().flatten().collect();

    if all_files.is_empty() {
        return Vec::new();
    }

    // Build thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.io_threads)
        .build()
        .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

    pool.install(|| {
        all_files
            .into_par_iter()
            .filter_map(|file| {
                if config.is_shutdown_requested() {
                    return None;
                }

                match hasher.prehash(&file.path) {
                    Ok(prehash) => Some(PrehashEntry { file, prehash }),
                    Err(e) => {
                        log::warn!("Failed to prehash {}: {}", file.path.display(), e);
                        None
                    }
                }
            })
            .collect()
    })
}

/// Get paths from a prehash group.
#[must_use]
pub fn extract_paths(files: &[FileEntry]) -> Vec<PathBuf> {
    files.iter().map(|f| f.path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::time::SystemTime;
    use tempfile::TempDir;

    fn make_file_entry(path: &str, size: u64) -> FileEntry {
        FileEntry::new(std::path::PathBuf::from(path), size, SystemTime::now())
    }

    fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) -> FileEntry {
        let path = dir.path().join(name);
        let mut f = File::create(&path).unwrap();
        f.write_all(content).unwrap();
        FileEntry::new(path, content.len() as u64, SystemTime::now())
    }

    #[test]
    fn test_prehash_config_default() {
        let config = PrehashConfig::default();
        assert_eq!(config.io_threads, 4);
        assert!(config.shutdown_flag.is_none());
        assert!(config.progress_callback.is_none());
    }

    #[test]
    fn test_prehash_config_builder() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let config = PrehashConfig::default()
            .with_io_threads(8)
            .with_shutdown_flag(shutdown.clone());

        assert_eq!(config.io_threads, 8);
        assert!(config.shutdown_flag.is_some());
    }

    #[test]
    fn test_prehash_stats_default() {
        let stats = PrehashStats::default();
        assert_eq!(stats.input_files, 0);
        assert_eq!(stats.hashed_files, 0);
        assert_eq!(stats.elimination_rate(), 0.0);
    }

    #[test]
    fn test_prehash_stats_elimination_rate() {
        let stats = PrehashStats {
            input_files: 100,
            hashed_files: 100,
            failed_files: 0,
            unique_prehashes: 80,
            potential_duplicates: 20,
            duplicate_groups: 5,
            interrupted: false,
        };

        assert!((stats.elimination_rate() - 80.0).abs() < 0.1);
    }

    #[test]
    fn test_phase2_empty_input() {
        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let (groups, stats) = phase2_prehash(HashMap::new(), hasher, config);

        assert!(groups.is_empty());
        assert_eq!(stats.input_files, 0);
        assert_eq!(stats.potential_duplicates, 0);
    }

    #[test]
    fn test_phase2_identical_files() {
        let dir = TempDir::new().unwrap();
        let content = b"identical content for testing";

        let file1 = create_test_file(&dir, "file1.txt", content);
        let file2 = create_test_file(&dir, "file2.txt", content);

        let mut size_groups = HashMap::new();
        size_groups.insert(content.len() as u64, vec![file1, file2]);

        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let (groups, stats) = phase2_prehash(size_groups, hasher, config);

        // Both files should be in same prehash group
        assert_eq!(groups.len(), 1);
        assert_eq!(stats.input_files, 2);
        assert_eq!(stats.potential_duplicates, 2);
        assert_eq!(stats.duplicate_groups, 1);
    }

    #[test]
    fn test_phase2_different_files() {
        let dir = TempDir::new().unwrap();

        // Same size, different content
        let file1 = create_test_file(&dir, "file1.txt", b"content A is here");
        let file2 = create_test_file(&dir, "file2.txt", b"content B is here");

        let mut size_groups = HashMap::new();
        size_groups.insert(17, vec![file1, file2]);

        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let (groups, stats) = phase2_prehash(size_groups, hasher, config);

        // Files have different prehashes, both eliminated
        assert!(groups.is_empty());
        assert_eq!(stats.input_files, 2);
        assert_eq!(stats.potential_duplicates, 0);
        assert_eq!(stats.unique_prehashes, 2);
    }

    #[test]
    fn test_phase2_mixed_files() {
        let dir = TempDir::new().unwrap();

        // Two identical files
        let file1 = create_test_file(&dir, "dup1.txt", b"duplicate content");
        let file2 = create_test_file(&dir, "dup2.txt", b"duplicate content");

        // One unique file (same size but different content)
        let file3 = create_test_file(&dir, "unique.txt", b"uniqueee content");

        let mut size_groups = HashMap::new();
        size_groups.insert(17, vec![file1, file2, file3]);

        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let (groups, stats) = phase2_prehash(size_groups, hasher, config);

        // One group with duplicates, one unique eliminated
        assert_eq!(groups.len(), 1);
        assert_eq!(stats.input_files, 3);
        assert_eq!(stats.potential_duplicates, 2);
        assert_eq!(stats.unique_prehashes, 1);
    }

    #[test]
    fn test_phase2_handles_missing_file() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "exists.txt", b"real content");

        // Create entry for non-existent file
        let file2 = make_file_entry(dir.path().join("missing.txt").to_str().unwrap(), 12);

        let mut size_groups = HashMap::new();
        size_groups.insert(12, vec![file1, file2]);

        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let (groups, stats) = phase2_prehash(size_groups, hasher, config);

        // Missing file should fail, existing file becomes unique
        assert!(groups.is_empty());
        assert_eq!(stats.input_files, 2);
        assert_eq!(stats.hashed_files, 1);
        assert_eq!(stats.failed_files, 1);
    }

    #[test]
    fn test_phase2_shutdown_flag() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "file1.txt", b"content");
        let file2 = create_test_file(&dir, "file2.txt", b"content");

        let mut size_groups = HashMap::new();
        size_groups.insert(7, vec![file1, file2]);

        let shutdown = Arc::new(AtomicBool::new(true)); // Already shutdown
        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default().with_shutdown_flag(shutdown);
        let (_, stats) = phase2_prehash(size_groups, hasher, config);

        // Should be interrupted
        assert!(stats.interrupted);
    }

    #[test]
    fn test_phase2_multiple_size_groups() {
        let dir = TempDir::new().unwrap();

        // Size group 1: 10 bytes, two identical files
        let file1 = create_test_file(&dir, "a1.txt", b"1234567890");
        let file2 = create_test_file(&dir, "a2.txt", b"1234567890");

        // Size group 2: 5 bytes, two identical files
        let file3 = create_test_file(&dir, "b1.txt", b"12345");
        let file4 = create_test_file(&dir, "b2.txt", b"12345");

        let mut size_groups = HashMap::new();
        size_groups.insert(10, vec![file1, file2]);
        size_groups.insert(5, vec![file3, file4]);

        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let (groups, stats) = phase2_prehash(size_groups, hasher, config);

        // Should have 2 prehash groups (one per size group)
        assert_eq!(groups.len(), 2);
        assert_eq!(stats.input_files, 4);
        assert_eq!(stats.potential_duplicates, 4);
        assert_eq!(stats.duplicate_groups, 2);
    }

    #[test]
    fn test_compute_prehashes() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "file1.txt", b"test content");
        let file2 = create_test_file(&dir, "file2.txt", b"test content");

        let mut size_groups = HashMap::new();
        size_groups.insert(12, vec![file1, file2]);

        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default();
        let entries = compute_prehashes(size_groups, hasher, config);

        assert_eq!(entries.len(), 2);
        // Both should have same prehash
        assert_eq!(entries[0].prehash, entries[1].prehash);
    }

    #[test]
    fn test_extract_paths() {
        let files = vec![
            make_file_entry("/path/a.txt", 100),
            make_file_entry("/path/b.txt", 100),
        ];

        let paths = extract_paths(&files);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], std::path::PathBuf::from("/path/a.txt"));
        assert_eq!(paths[1], std::path::PathBuf::from("/path/b.txt"));
    }

    struct TestProgressCallback {
        phase_started: std::sync::Mutex<bool>,
        progress_count: std::sync::atomic::AtomicUsize,
        phase_ended: std::sync::Mutex<bool>,
    }

    impl TestProgressCallback {
        fn new() -> Self {
            Self {
                phase_started: std::sync::Mutex::new(false),
                progress_count: std::sync::atomic::AtomicUsize::new(0),
                phase_ended: std::sync::Mutex::new(false),
            }
        }
    }

    impl ProgressCallback for TestProgressCallback {
        fn on_phase_start(&self, _phase: &str, _total: usize) {
            *self.phase_started.lock().unwrap() = true;
        }

        fn on_progress(&self, _current: usize, _path: &str) {
            self.progress_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn on_phase_end(&self, _phase: &str) {
            *self.phase_ended.lock().unwrap() = true;
        }
    }

    #[test]
    fn test_phase2_progress_callback() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "file1.txt", b"content");
        let file2 = create_test_file(&dir, "file2.txt", b"content");

        let mut size_groups = HashMap::new();
        size_groups.insert(7, vec![file1, file2]);

        let callback = Arc::new(TestProgressCallback::new());
        let hasher = Arc::new(Hasher::new());
        let config = PrehashConfig::default().with_progress_callback(callback.clone());

        let (_, _) = phase2_prehash(size_groups, hasher, config);

        // Callback should have been called
        assert!(*callback.phase_started.lock().unwrap());
        assert!(callback.progress_count.load(Ordering::SeqCst) > 0);
        assert!(*callback.phase_ended.lock().unwrap());
    }
}
