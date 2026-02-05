//! Duplicate finder implementation with multi-phase detection.
//!
//! # Overview
//!
//! This module orchestrates the duplicate detection pipeline:
//! 1. **Phase 1 - Size grouping**: Group files by size (see [`crate::duplicates::groups`] module)
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

use growable_bloom_filter::GrowableBloom;
use rayon::prelude::*;

use crate::cache::{CacheEntry, HashCache};
use crate::progress::ProgressCallback;
use crate::scanner::{FileEntry, Hash, Hasher};

/// Configuration for prehash phase.
#[derive(Clone)]
pub struct PrehashConfig {
    /// Number of I/O threads for parallel hashing.
    /// Default is 4 to prevent disk thrashing.
    pub io_threads: usize,
    /// Optional hash cache for faster rescans.
    pub cache: Option<Arc<HashCache>>,
    /// Optional shutdown flag for graceful termination.
    pub shutdown_flag: Option<Arc<AtomicBool>>,
    /// Optional progress callback.
    pub progress_callback: Option<Arc<dyn ProgressCallback>>,
    /// Protected reference paths.
    pub reference_paths: Vec<PathBuf>,
    /// False positive rate for Bloom filters.
    pub bloom_fp_rate: f64,
}

impl std::fmt::Debug for PrehashConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrehashConfig")
            .field("io_threads", &self.io_threads)
            .field("cache", &self.cache.as_ref().map(|_| "<cache>"))
            .field("shutdown_flag", &self.shutdown_flag)
            .field(
                "progress_callback",
                &self.progress_callback.as_ref().map(|_| "<callback>"),
            )
            .field("reference_paths", &self.reference_paths)
            .field("bloom_fp_rate", &self.bloom_fp_rate)
            .finish()
    }
}

impl Default for PrehashConfig {
    fn default() -> Self {
        Self {
            io_threads: 4,
            cache: None,
            shutdown_flag: None,
            progress_callback: None,
            reference_paths: Vec::new(),
            bloom_fp_rate: 0.01,
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

    /// Set the hash cache.
    #[must_use]
    pub fn with_cache(mut self, cache: Arc<HashCache>) -> Self {
        self.cache = Some(cache);
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

    /// Set the reference paths for protecting directories.
    #[must_use]
    pub fn with_reference_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.reference_paths = paths;
        self
    }

    /// Set the Bloom filter false positive rate.
    #[must_use]
    pub fn with_bloom_fp_rate(mut self, rate: f64) -> Self {
        self.bloom_fp_rate = rate;
        self
    }

    /// Check if shutdown has been requested.
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }
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
    /// Errors encountered during prehash
    pub errors: Vec<crate::scanner::HashError>,
    /// Number of cache hits for prehashes
    pub cache_hits: usize,
    /// Number of cache misses for prehashes
    pub cache_misses: usize,
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
    let prehash_results: Vec<(
        FileEntry,
        Result<Hash, crate::scanner::HashError>,
        bool,
        bool,
    )> = pool.install(|| {
        all_files
            .into_par_iter()
            .enumerate()
            .map(|(idx, file)| {
                // Check shutdown flag
                if config.is_shutdown_requested() {
                    log::debug!("Phase 2: Shutdown requested, skipping remaining files");
                    return (
                        file,
                        Err(crate::scanner::HashError::Io {
                            path: PathBuf::new(),
                            source: Arc::new(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "Shutdown",
                            )),
                        }),
                        false,
                        true,
                    );
                }

                // Report progress
                if let Some(ref callback) = config.progress_callback {
                    callback.on_progress(idx + 1, file.path.to_string_lossy().as_ref());
                }

                // Check cache first
                if let Some(ref cache) = config.cache {
                    match cache.get_prehash(&file.path, file.size, file.modified) {
                        Ok(Some(hash)) => {
                            log::trace!("Prehash cache hit: {}", file.path.display());
                            return (file, Ok(hash), true, false);
                        }
                        Ok(None) => {
                            log::trace!("Prehash cache miss: {}", file.path.display());
                        }
                        Err(e) => {
                            log::warn!("Failed to query cache for {}: {}", file.path.display(), e);
                        }
                    }
                }

                // Compute prehash
                match hasher.prehash(&file.path) {
                    Ok(hash) => {
                        log::trace!("Prehash computed: {}", file.path.display());

                        // Update cache
                        if let Some(ref cache) = config.cache {
                            let entry = CacheEntry::from(file.clone());
                            if let Err(e) = cache.insert_prehash(&entry, hash) {
                                log::warn!(
                                    "Failed to update cache for {}: {}",
                                    file.path.display(),
                                    e
                                );
                            }
                        }

                        (file, Ok(hash), false, false)
                    }
                    Err(e) => {
                        log::warn!("Failed to prehash {}: {}", file.path.display(), e);
                        (file, Err(e), false, false)
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
    let mut seen_prehashes = GrowableBloom::new(config.bloom_fp_rate, prehash_results.len());
    let mut duplicate_prehashes = GrowableBloom::new(config.bloom_fp_rate, prehash_results.len());
    let mut first_prehash_occurrences: HashMap<Hash, FileEntry> = HashMap::new();

    for (file, res, is_hit, is_interrupted) in prehash_results {
        if is_interrupted {
            continue;
        }
        match res {
            Ok(prehash) => {
                stats.hashed_files += 1;
                if is_hit {
                    stats.cache_hits += 1;
                } else {
                    stats.cache_misses += 1;
                }

                if duplicate_prehashes.contains(prehash) {
                    prehash_groups.entry(prehash).or_default().push(file);
                } else if seen_prehashes.contains(prehash) {
                    duplicate_prehashes.insert(prehash);
                    if let Some(first) = first_prehash_occurrences.remove(&prehash) {
                        prehash_groups.entry(prehash).or_default().push(first);
                    }
                    prehash_groups.entry(prehash).or_default().push(file);
                } else {
                    seen_prehashes.insert(prehash);
                    first_prehash_occurrences.insert(prehash, file);
                }
            }
            Err(e) => {
                stats.failed_files += 1;
                stats.errors.push(e);
            }
        }
    }

    // Filter to only groups with 2+ files (potential duplicates)
    // Actually, with Bloom filter logic above, prehash_groups ALREADY only contains groups with 2+ files
    // But there might be false positives from duplicate_prehashes Bloom filter.
    // If duplicate_prehashes.contains(&prehash) was true but it was a false positive,
    // we might have a group with only 1 file in prehash_groups.
    // So we still need to filter.

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

    // Add unique prehashes from first_prehash_occurrences to stats
    stats.unique_prehashes += first_prehash_occurrences.len();

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
/// # Example
///
/// ```no_run
/// use rustdupe::scanner::{FileEntry, Hasher};
/// use rustdupe::duplicates::{compute_prehashes, PrehashConfig};
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// let size_groups: HashMap<u64, Vec<FileEntry>> = HashMap::new();
/// let hasher = Arc::new(Hasher::new());
/// let entries = compute_prehashes(size_groups, hasher, PrehashConfig::default());
/// ```
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

                // Check cache first
                if let Some(ref cache) = config.cache {
                    if let Ok(Some(prehash)) =
                        cache.get_prehash(&file.path, file.size, file.modified)
                    {
                        return Some(PrehashEntry { file, prehash });
                    }
                }

                match hasher.prehash(&file.path) {
                    Ok(prehash) => {
                        // Update cache
                        if let Some(ref cache) = config.cache {
                            let entry = CacheEntry::from(file.clone());
                            let _ = cache.insert_prehash(&entry, prehash);
                        }
                        Some(PrehashEntry { file, prehash })
                    }
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
/// # Example
///
/// ```
/// use rustdupe::scanner::FileEntry;
/// use rustdupe::duplicates::extract_paths;
/// use std::path::PathBuf;
/// use std::time::SystemTime;
///
/// let files = vec![
///     FileEntry::new(PathBuf::from("/a.txt"), 100, SystemTime::now()),
/// ];
/// let paths = extract_paths(&files);
/// assert_eq!(paths[0], PathBuf::from("/a.txt"));
/// ```
#[must_use]
pub fn extract_paths(files: &[FileEntry]) -> Vec<PathBuf> {
    files.iter().map(|f| f.path.clone()).collect()
}

/// Threshold for logging large files.
const LARGE_FILE_THRESHOLD: u64 = 100 * 1024 * 1024; // 100MB

/// Configuration for full hash phase.
#[derive(Clone)]
pub struct FullhashConfig {
    /// Number of I/O threads for parallel hashing.
    /// Default is 4 to prevent disk thrashing.
    pub io_threads: usize,
    /// Optional hash cache for faster rescans.
    pub cache: Option<Arc<HashCache>>,
    /// Optional shutdown flag for graceful termination.
    pub shutdown_flag: Option<Arc<AtomicBool>>,
    /// Optional progress callback.
    pub progress_callback: Option<Arc<dyn ProgressCallback>>,
    /// Protected reference paths.
    pub reference_paths: Vec<PathBuf>,
}

impl std::fmt::Debug for FullhashConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FullhashConfig")
            .field("io_threads", &self.io_threads)
            .field("cache", &self.cache.as_ref().map(|_| "<cache>"))
            .field("shutdown_flag", &self.shutdown_flag)
            .field(
                "progress_callback",
                &self.progress_callback.as_ref().map(|_| "<callback>"),
            )
            .field("reference_paths", &self.reference_paths)
            .finish()
    }
}

impl Default for FullhashConfig {
    fn default() -> Self {
        Self {
            io_threads: 4,
            cache: None,
            shutdown_flag: None,
            progress_callback: None,
            reference_paths: Vec::new(),
        }
    }
}

impl FullhashConfig {
    /// Create a new configuration with custom I/O thread count.
    #[must_use]
    pub fn with_io_threads(mut self, threads: usize) -> Self {
        self.io_threads = threads.max(1);
        self
    }

    /// Set the hash cache.
    #[must_use]
    pub fn with_cache(mut self, cache: Arc<HashCache>) -> Self {
        self.cache = Some(cache);
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

    /// Set the reference paths for protecting directories.
    #[must_use]
    pub fn with_reference_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.reference_paths = paths;
        self
    }

    /// Check if shutdown has been requested.
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }
}

/// Statistics from full hash phase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FullhashStats {
    /// Total files that entered Phase 3
    pub input_files: usize,
    /// Number of files successfully hashed
    pub hashed_files: usize,
    /// Number of files that failed to hash (I/O errors)
    pub failed_files: usize,
    /// Errors encountered during full hash
    pub errors: Vec<crate::scanner::HashError>,
    /// Number of cache hits for full hashes
    pub cache_hits: usize,
    /// Number of cache misses for full hashes
    pub cache_misses: usize,
    /// Total bytes hashed across all files
    pub bytes_hashed: u64,
    /// Number of confirmed duplicate groups
    pub duplicate_groups: usize,
    /// Number of confirmed duplicate files (excluding originals)
    pub duplicate_files: usize,
    /// Total space wasted by duplicates
    pub wasted_space: u64,
    /// Whether phase was interrupted by shutdown
    pub interrupted: bool,
}

impl FullhashStats {
    /// Calculate wasted space from duplicate groups.
    pub fn calculate_wasted_space(&mut self, groups: &[super::DuplicateGroup]) {
        self.duplicate_groups = groups.len();
        self.duplicate_files = groups.iter().map(|g| g.duplicate_count()).sum();
        self.wasted_space = groups.iter().map(|g| g.wasted_space()).sum();
    }
}

/// Compute full hashes for prehash groups (Phase 3).
///
/// This is the third and final phase of duplicate detection. For each prehash
/// group, it computes the full content hash of each file to confirm that they
/// are true duplicates.
///
/// # Arguments
///
/// * `prehash_groups` - Files grouped by prehash from Phase 2
/// * `hasher` - The hasher to use for computing full hashes
/// * `config` - Configuration for the full hash phase
///
/// # Returns
///
/// A tuple of:
/// - `Vec<DuplicateGroup>` - Confirmed duplicate groups
/// - `FullhashStats` - Statistics about the full hash operation
///
/// # Performance
///
/// - Uses rayon for parallel I/O (limited to `io_threads` to prevent disk thrashing)
/// - Streams entire file content (O(total bytes))
/// - Large files (>100MB) are logged at debug level for visibility
///
/// # Example
///
/// ```no_run
/// use rustdupe::scanner::{FileEntry, Hasher};
/// use rustdupe::duplicates::{phase2_prehash, phase3_fullhash, PrehashConfig, FullhashConfig};
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// // Assume prehash_groups from Phase 2
/// let prehash_groups: HashMap<[u8; 32], Vec<FileEntry>> = HashMap::new();
///
/// let hasher = Arc::new(Hasher::new());
/// let config = FullhashConfig::default();
/// let (duplicate_groups, stats) = phase3_fullhash(prehash_groups, hasher, config);
///
/// println!("Found {} duplicate groups wasting {} bytes",
///     stats.duplicate_groups, stats.wasted_space);
/// ```
#[must_use]
pub fn phase3_fullhash(
    prehash_groups: HashMap<Hash, Vec<FileEntry>>,
    hasher: Arc<Hasher>,
    config: FullhashConfig,
) -> (Vec<super::DuplicateGroup>, FullhashStats) {
    // Count total input files
    let input_files: usize = prehash_groups.values().map(|v| v.len()).sum();
    let mut stats = FullhashStats {
        input_files,
        ..Default::default()
    };

    // Flatten all files from prehash groups, preserving the prehash
    let all_files: Vec<(FileEntry, Hash)> = prehash_groups
        .into_iter()
        .flat_map(|(hash, files)| files.into_iter().map(move |f| (f, hash)))
        .collect();

    if all_files.is_empty() {
        log::debug!("Phase 3: No files to process");
        return (Vec::new(), stats);
    }

    // Notify progress callback
    if let Some(ref callback) = config.progress_callback {
        callback.on_phase_start("fullhash", all_files.len());
    }

    log::info!(
        "Phase 3: Computing full hashes for {} files",
        all_files.len()
    );

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

    // Compute full hashes in parallel with limited I/O parallelism
    let hash_results: Vec<(
        FileEntry,
        Result<Hash, crate::scanner::HashError>,
        bool,
        bool,
    )> = pool.install(|| {
        all_files
            .into_par_iter()
            .enumerate()
            .map(|(idx, (file, prehash))| {
                // Check shutdown flag
                if config.is_shutdown_requested() {
                    log::debug!("Phase 3: Shutdown requested, skipping remaining files");
                    return (
                        file,
                        Err(crate::scanner::HashError::Io {
                            path: PathBuf::new(),
                            source: Arc::new(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "Shutdown",
                            )),
                        }),
                        false,
                        true,
                    );
                }

                // Log large files for visibility
                if file.size > LARGE_FILE_THRESHOLD {
                    log::debug!(
                        "Hashing large file ({} MB): {}",
                        file.size / (1024 * 1024),
                        file.path.display()
                    );
                }

                // Report progress
                if let Some(ref callback) = config.progress_callback {
                    callback.on_progress(idx + 1, file.path.to_string_lossy().as_ref());
                }

                // Check cache first
                if let Some(ref cache) = config.cache {
                    match cache.get_fullhash(&file.path, file.size, file.modified) {
                        Ok(Some(hash)) => {
                            log::trace!("Full hash cache hit: {}", file.path.display());
                            return (file, Ok(hash), true, false);
                        }
                        Ok(None) => {
                            log::trace!("Full hash cache miss: {}", file.path.display());
                        }
                        Err(e) => {
                            log::warn!("Failed to query cache for {}: {}", file.path.display(), e);
                        }
                    }
                }

                // Compute full hash
                match hasher.full_hash(&file.path) {
                    Ok(hash) => {
                        log::trace!("Full hash computed: {}", file.path.display());
                        if let Some(ref callback) = config.progress_callback {
                            callback.on_item_completed(file.size);
                        }

                        // Update cache
                        if let Some(ref cache) = config.cache {
                            let mut entry = CacheEntry::from(file.clone());
                            entry.prehash = prehash;
                            if let Err(e) = cache.insert_fullhash(&entry, hash) {
                                log::warn!(
                                    "Failed to update cache for {}: {}",
                                    file.path.display(),
                                    e
                                );
                            }
                        }

                        (file, Ok(hash), false, false)
                    }
                    Err(e) => {
                        log::warn!("Failed to hash {}: {}", file.path.display(), e);
                        (file, Err(e), false, false)
                    }
                }
            })
            .collect()
    });

    // Check if we were interrupted
    if config.is_shutdown_requested() {
        stats.interrupted = true;
        log::info!("Phase 3: Interrupted by shutdown signal");
    }

    // Group by full hash
    let mut fullhash_groups: HashMap<Hash, Vec<FileEntry>> = HashMap::new();

    for (file, res, is_hit, is_interrupted) in hash_results {
        if is_interrupted {
            continue;
        }
        match res {
            Ok(fullhash) => {
                stats.hashed_files += 1;
                stats.bytes_hashed += file.size;
                if is_hit {
                    stats.cache_hits += 1;
                } else {
                    stats.cache_misses += 1;
                }
                fullhash_groups.entry(fullhash).or_default().push(file);
            }
            Err(e) => {
                stats.failed_files += 1;
                stats.errors.push(e);
            }
        }
    }

    // Convert to DuplicateGroup structs, filtering to only groups with 2+ files
    let duplicate_groups: Vec<super::DuplicateGroup> = fullhash_groups
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(hash, files)| {
            let size = files.first().map_or(0, |f| f.size);
            log::debug!(
                "Duplicate group {}: {} files, {} bytes each",
                crate::scanner::hash_to_hex(&hash),
                files.len(),
                size
            );
            super::DuplicateGroup::new(hash, size, files, config.reference_paths.clone())
        })
        .collect();

    // Calculate final statistics
    stats.calculate_wasted_space(&duplicate_groups);

    // Notify progress callback
    if let Some(ref callback) = config.progress_callback {
        callback.on_phase_end("fullhash");
    }

    log::info!(
        "Phase 3 complete: {} groups, {} duplicates, {} bytes reclaimable",
        stats.duplicate_groups,
        stats.duplicate_files,
        stats.wasted_space
    );

    (duplicate_groups, stats)
}

// ============================================================================
// DuplicateFinder - Pipeline Orchestrator
// ============================================================================

/// Configuration for the duplicate finder.
///
/// Controls the behavior of the multi-phase duplicate detection pipeline.
#[derive(Clone)]
pub struct FinderConfig {
    /// Number of I/O threads for parallel hashing.
    /// Default is 4 to prevent disk thrashing.
    pub io_threads: usize,
    /// Fail-fast on any error during scan.
    pub strict: bool,
    /// Optional hash cache for faster rescans.
    pub cache: Option<Arc<HashCache>>,
    /// Enable byte-by-byte verification after hash matching (paranoid mode).
    pub paranoid: bool,
    /// Walker configuration for directory traversal.
    pub walker_config: crate::scanner::WalkerConfig,
    /// Optional shutdown flag for graceful termination.
    pub shutdown_flag: Option<Arc<AtomicBool>>,
    /// Optional progress callback for reporting.
    pub progress_callback: Option<Arc<dyn ProgressCallback>>,
    /// Protected reference paths.
    pub reference_paths: Vec<PathBuf>,
    /// Named directory groups mapping canonical paths to group names.
    pub group_map: std::collections::HashMap<PathBuf, String>,
    /// False positive rate for Bloom filters (default: 0.01).
    pub bloom_fp_rate: f64,
}

impl std::fmt::Debug for FinderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinderConfig")
            .field("io_threads", &self.io_threads)
            .field("cache", &self.cache.as_ref().map(|_| "<cache>"))
            .field("paranoid", &self.paranoid)
            .field("walker_config", &self.walker_config)
            .field("shutdown_flag", &self.shutdown_flag)
            .field(
                "progress_callback",
                &self.progress_callback.as_ref().map(|_| "<callback>"),
            )
            .field("reference_paths", &self.reference_paths)
            .field("group_map", &self.group_map)
            .field("bloom_fp_rate", &self.bloom_fp_rate)
            .finish()
    }
}

impl Default for FinderConfig {
    fn default() -> Self {
        Self {
            io_threads: 4,
            strict: false,
            cache: None,
            paranoid: false,
            walker_config: crate::scanner::WalkerConfig::default(),
            shutdown_flag: None,
            progress_callback: None,
            reference_paths: Vec::new(),
            group_map: std::collections::HashMap::new(),
            bloom_fp_rate: 0.01,
        }
    }
}

impl FinderConfig {
    /// Create a new configuration with custom I/O thread count.
    #[must_use]
    pub fn with_io_threads(mut self, threads: usize) -> Self {
        self.io_threads = threads.max(1);
        self
    }

    /// Set fail-fast on any error.
    #[must_use]
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Set the hash cache.
    #[must_use]
    pub fn with_cache(mut self, cache: Arc<HashCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Enable paranoid mode (byte-by-byte verification).
    #[must_use]
    pub fn with_paranoid(mut self, enabled: bool) -> Self {
        self.paranoid = enabled;
        self
    }

    /// Set the walker configuration.
    #[must_use]
    pub fn with_walker_config(mut self, config: crate::scanner::WalkerConfig) -> Self {
        self.walker_config = config;
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

    /// Set the reference paths for protecting directories.
    #[must_use]
    pub fn with_reference_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.reference_paths = paths;
        self
    }

    /// Set the group map for named directory groups.
    #[must_use]
    pub fn with_group_map(mut self, map: std::collections::HashMap<PathBuf, String>) -> Self {
        self.group_map = map;
        self
    }

    /// Set the Bloom filter false positive rate.
    #[must_use]
    pub fn with_bloom_fp_rate(mut self, rate: f64) -> Self {
        self.bloom_fp_rate = rate.clamp(0.0001, 0.1);
        self
    }

    /// Check if shutdown has been requested.
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }
}

/// Summary statistics from a duplicate scan.
///
/// Provides comprehensive metrics about the scan results including
/// file counts, sizes, and potential space savings.
#[derive(Debug, Clone, Default)]
pub struct ScanSummary {
    /// Total number of files scanned
    pub total_files: usize,
    /// Total size of all scanned files in bytes
    pub total_size: u64,
    /// Number of files eliminated by size grouping (unique sizes)
    pub eliminated_by_size: usize,
    /// Number of files eliminated by prehash (different first 4KB)
    pub eliminated_by_prehash: usize,
    /// Number of cache hits for prehashes
    pub cache_prehash_hits: usize,
    /// Number of cache misses for prehashes
    pub cache_prehash_misses: usize,
    /// Number of cache hits for full hashes
    pub cache_fullhash_hits: usize,
    /// Number of cache misses for full hashes
    pub cache_fullhash_misses: usize,
    /// Number of confirmed duplicate groups
    pub duplicate_groups: usize,
    /// Total number of duplicate files (excluding originals)
    pub duplicate_files: usize,
    /// Total space that can be reclaimed by removing duplicates
    pub reclaimable_space: u64,
    /// Duration of the entire scan
    pub scan_duration: std::time::Duration,
    /// Whether the scan was interrupted
    pub interrupted: bool,
    /// Errors encountered during the scan
    pub scan_errors: Vec<crate::scanner::ScanError>,
}

impl ScanSummary {
    /// Calculate the percentage of space that is wasted by duplicates.
    #[must_use]
    pub fn wasted_percentage(&self) -> f64 {
        if self.total_size == 0 {
            0.0
        } else {
            (self.reclaimable_space as f64 / self.total_size as f64) * 100.0
        }
    }

    /// Format reclaimable space as human-readable string.
    #[must_use]
    pub fn reclaimable_display(&self) -> String {
        format_size(self.reclaimable_space)
    }

    /// Format total size as human-readable string.
    #[must_use]
    pub fn total_size_display(&self) -> String {
        format_size(self.total_size)
    }
}

/// Format a byte size as a human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Errors that can occur during duplicate finding.
#[derive(thiserror::Error, Debug)]
pub enum FinderError {
    /// The scan was interrupted by user (Ctrl+C or shutdown signal).
    #[error("Scan interrupted by user")]
    Interrupted,

    /// The provided path does not exist.
    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    /// The provided path is not a directory.
    #[error("Not a directory: {0}")]
    NotADirectory(PathBuf),

    /// An I/O error occurred during scanning.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An I/O error occurred during scanning with a specific path.
    #[error("I/O error for {path}: {source}")]
    IoWithPath {
        /// Path where the error occurred
        path: PathBuf,
        /// The underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// A scan error occurred.
    #[error(transparent)]
    ScanError(#[from] crate::scanner::ScanError),
}

/// Duplicate finder that orchestrates the multi-phase detection pipeline.
///
/// The `DuplicateFinder` runs the complete duplicate detection pipeline:
/// 1. **Walk** - Collect all files from the target directory
/// 2. **Phase 1** - Group files by size (eliminates 70-90%)
/// 3. **Phase 2** - Compare prehashes of same-size files (eliminates 80-95%)
/// 4. **Phase 3** - Compute full hashes to confirm duplicates
///
/// # Example
///
/// ```no_run
/// use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
/// use std::path::Path;
///
/// let config = FinderConfig::default().with_io_threads(4);
/// let finder = DuplicateFinder::new(config);
///
/// let (groups, summary) = finder.find_duplicates(Path::new("/some/path")).unwrap();
///
/// println!("Found {} duplicate groups", summary.duplicate_groups);
/// println!("Reclaimable space: {}", summary.reclaimable_display());
/// ```
pub struct DuplicateFinder {
    config: FinderConfig,
    hasher: Arc<Hasher>,
}

impl DuplicateFinder {
    /// Create a new duplicate finder with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the finder
    #[must_use]
    pub fn new(config: FinderConfig) -> Self {
        let mut hasher = Hasher::new();
        if let Some(ref flag) = config.shutdown_flag {
            hasher = hasher.with_shutdown_flag(flag.clone());
        }
        Self {
            config,
            hasher: Arc::new(hasher),
        }
    }

    /// Create a new duplicate finder with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(FinderConfig::default())
    }

    /// Find all duplicate files starting from the given path.
    ///
    /// Runs the complete multi-phase duplicate detection pipeline and
    /// returns confirmed duplicate groups along with summary statistics.
    ///
    /// # Arguments
    ///
    /// * `path` - Root directory to scan for duplicates
    ///
    /// # Returns
    ///
    /// A tuple of:
    /// - `Vec<DuplicateGroup>` - Confirmed duplicate groups
    /// - `ScanSummary` - Statistics about the scan
    ///
    /// # Errors
    ///
    /// Returns `FinderError` if:
    /// - The path does not exist
    /// - The path is not a directory
    /// - The scan is interrupted by shutdown signal
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
    /// use std::path::Path;
    ///
    /// let finder = DuplicateFinder::with_defaults();
    /// match finder.find_duplicates(Path::new(".")) {
    ///     Ok((groups, summary)) => {
    ///         println!("Found {} duplicate groups", groups.len());
    ///         println!("Can reclaim {} bytes", summary.reclaimable_space);
    ///     }
    ///     Err(e) => eprintln!("Scan failed: {}", e),
    /// }
    /// ```
    pub fn find_duplicates(
        &self,
        path: &std::path::Path,
    ) -> Result<(Vec<super::DuplicateGroup>, ScanSummary), FinderError> {
        let start_time = std::time::Instant::now();
        let mut summary = ScanSummary::default();

        // Validate path
        if !path.exists() {
            return Err(FinderError::PathNotFound(path.to_path_buf()));
        }
        if !path.is_dir() {
            return Err(FinderError::NotADirectory(path.to_path_buf()));
        }

        log::info!("Starting duplicate scan of {}", path.display());

        // Check for early shutdown
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        // Phase 0: Walk directory and collect files
        if let Some(ref callback) = self.config.progress_callback {
            callback.on_phase_start("walking", 0);
            callback.on_message(&format!("Walking {}", path.display()));
        }

        let mut walker = crate::scanner::Walker::new(path, self.config.walker_config.clone());

        // Set shutdown flag on walker if available
        if let Some(ref flag) = self.config.shutdown_flag {
            walker = walker.with_shutdown_flag(flag.clone());
        }

        // Set progress callback on walker if available
        if let Some(ref callback) = self.config.progress_callback {
            walker = walker.with_progress_callback(callback.clone());
        }

        let mut files = Vec::new();
        let mut seen_sizes = GrowableBloom::new(self.config.bloom_fp_rate, 1000);
        let mut duplicate_sizes = GrowableBloom::new(self.config.bloom_fp_rate, 1000);
        let mut first_occurrences: HashMap<u64, FileEntry> = HashMap::new();

        for result in walker.walk() {
            match result {
                Ok(file) => {
                    // Empty files are handled separately by group_by_size
                    if file.size == 0 {
                        files.push(file);
                        continue;
                    }

                    if duplicate_sizes.contains(file.size) {
                        files.push(file);
                    } else if seen_sizes.contains(file.size) {
                        duplicate_sizes.insert(file.size);
                        if let Some(first) = first_occurrences.remove(&file.size) {
                            files.push(first);
                        }
                        files.push(file);
                    } else {
                        seen_sizes.insert(file.size);
                        first_occurrences.insert(file.size, file);
                    }
                }
                Err(e) => {
                    if self.config.strict {
                        return Err(FinderError::ScanError(e));
                    } else {
                        summary.scan_errors.push(e);
                    }
                }
            }
        }

        // Add remaining first occurrences that were actually duplicates (false negatives from Bloom)
        // Actually, Bloom filter has no false negatives, only false positives.
        // So any size not in duplicate_sizes is either unique or a false positive from seen_sizes.
        // If it's a false positive, it means we thought we saw it, but we didn't.
        // Wait, Bloom filter 'contains' returning true means it MIGHT be there.
        // If seen_sizes.contains(&file.size) returns true, but it's a false positive,
        // we'll add it to duplicate_sizes and move it to 'files'.
        // This is safe, it just means we might keep a few unique files.

        if let Some(ref callback) = self.config.progress_callback {
            callback.on_phase_end("walking");
        }

        // Summary counts should reflect what we actually found
        summary.total_files = files.len() + first_occurrences.len();
        summary.total_size = files.iter().map(|f| f.size).sum::<u64>()
            + first_occurrences.values().map(|f| f.size).sum::<u64>();

        log::info!(
            "Found {} files ({} total)",
            summary.total_files,
            format_size(summary.total_size)
        );

        // Check for shutdown after walking
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if files.is_empty() {
            log::info!("No potential duplicates found after size filtering, scan complete");
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 1: Group by size
        log::info!("Phase 1: Grouping by size...");
        // group_by_size will still handle the files we kept
        let (size_groups, size_stats) = super::group_by_size(files);

        // Update eliminated count to include files we discarded during walk
        summary.eliminated_by_size = size_stats.eliminated_unique + first_occurrences.len();

        log::info!(
            "Phase 1 complete: {} → {} files ({:.1}% eliminated)",
            size_stats.total_files,
            size_stats.potential_duplicates,
            size_stats.elimination_rate()
        );

        // Check for shutdown after Phase 1
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if size_groups.is_empty() {
            log::info!("No potential duplicates found after size grouping");
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 2: Prehash comparison
        log::info!("Phase 2: Computing prehashes...");
        let prehash_config = PrehashConfig {
            io_threads: self.config.io_threads,
            cache: self.config.cache.clone(),
            shutdown_flag: self.config.shutdown_flag.clone(),
            progress_callback: self.config.progress_callback.clone(),
            reference_paths: self.config.reference_paths.clone(),
            bloom_fp_rate: self.config.bloom_fp_rate,
        };

        let (prehash_groups, prehash_stats) =
            phase2_prehash(size_groups, self.hasher.clone(), prehash_config);

        summary.eliminated_by_prehash = prehash_stats.unique_prehashes;
        summary.cache_prehash_hits = prehash_stats.cache_hits;
        summary.cache_prehash_misses = prehash_stats.cache_misses;

        if !prehash_stats.errors.is_empty() {
            if self.config.strict {
                return Err(FinderError::ScanError(
                    crate::scanner::ScanError::HashError(prehash_stats.errors[0].clone()),
                ));
            } else {
                summary.scan_errors.extend(
                    prehash_stats
                        .errors
                        .into_iter()
                        .map(crate::scanner::ScanError::from),
                );
            }
        }

        if prehash_stats.interrupted || self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if prehash_groups.is_empty() {
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 3: Full hash comparison
        let fullhash_config = FullhashConfig {
            io_threads: self.config.io_threads,
            cache: self.config.cache.clone(),
            shutdown_flag: self.config.shutdown_flag.clone(),
            progress_callback: self.config.progress_callback.clone(),
            reference_paths: self.config.reference_paths.clone(),
        };

        let (duplicate_groups, fullhash_stats) =
            phase3_fullhash(prehash_groups, self.hasher.clone(), fullhash_config);

        if !fullhash_stats.errors.is_empty() {
            if self.config.strict {
                return Err(FinderError::ScanError(
                    crate::scanner::ScanError::HashError(fullhash_stats.errors[0].clone()),
                ));
            } else {
                summary.scan_errors.extend(
                    fullhash_stats
                        .errors
                        .into_iter()
                        .map(crate::scanner::ScanError::from),
                );
            }
        }

        if fullhash_stats.interrupted || self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        // Update summary
        summary.duplicate_groups = fullhash_stats.duplicate_groups;
        summary.duplicate_files = fullhash_stats.duplicate_files;
        summary.reclaimable_space = fullhash_stats.wasted_space;
        summary.cache_fullhash_hits = fullhash_stats.cache_hits;
        summary.cache_fullhash_misses = fullhash_stats.cache_misses;
        summary.scan_duration = start_time.elapsed();

        log::info!(
            "Scan complete: {} duplicate groups, {} duplicate files, {} reclaimable, {} cache hits",
            summary.duplicate_groups,
            summary.duplicate_files,
            summary.reclaimable_display(),
            summary.cache_prehash_hits + summary.cache_fullhash_hits
        );

        Ok((duplicate_groups, summary))
    }

    /// Find duplicates from a pre-collected list of files.
    ///
    /// Use this method when you already have a list of files from another source
    /// (e.g., a custom walker or cached file list).
    ///
    /// # Arguments
    ///
    /// * `files` - List of file entries to check for duplicates
    ///
    /// # Returns
    ///
    /// A tuple of:
    /// - `Vec<DuplicateGroup>` - Confirmed duplicate groups
    /// - `ScanSummary` - Statistics about the scan
    pub fn find_duplicates_from_files(
        &self,
        files: Vec<FileEntry>,
    ) -> Result<(Vec<super::DuplicateGroup>, ScanSummary), FinderError> {
        let start_time = std::time::Instant::now();
        let total_files = files.len();
        let total_size: u64 = files.iter().map(|f| f.size).sum();

        let mut summary = ScanSummary {
            total_files,
            total_size,
            ..Default::default()
        };

        log::info!(
            "Processing {} files ({})",
            files.len(),
            format_size(summary.total_size)
        );

        // Check for early shutdown
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if files.is_empty() {
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 1: Group by size
        let mut potential_files = Vec::new();
        let mut seen_sizes = GrowableBloom::new(self.config.bloom_fp_rate, files.len());
        let mut duplicate_sizes = GrowableBloom::new(self.config.bloom_fp_rate, files.len());
        let mut first_occurrences: HashMap<u64, FileEntry> = HashMap::new();

        for file in files {
            if file.size == 0 {
                potential_files.push(file);
                continue;
            }

            if duplicate_sizes.contains(file.size) {
                potential_files.push(file);
            } else if seen_sizes.contains(file.size) {
                duplicate_sizes.insert(file.size);
                if let Some(first) = first_occurrences.remove(&file.size) {
                    potential_files.push(first);
                }
                potential_files.push(file);
            } else {
                seen_sizes.insert(file.size);
                first_occurrences.insert(file.size, file);
            }
        }

        let (size_groups, size_stats) = super::group_by_size(potential_files);
        summary.eliminated_by_size = size_stats.eliminated_unique + first_occurrences.len();

        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if size_groups.is_empty() {
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 2: Prehash comparison
        let prehash_config = PrehashConfig {
            io_threads: self.config.io_threads,
            cache: self.config.cache.clone(),
            shutdown_flag: self.config.shutdown_flag.clone(),
            progress_callback: self.config.progress_callback.clone(),
            reference_paths: self.config.reference_paths.clone(),
            bloom_fp_rate: self.config.bloom_fp_rate,
        };

        let (prehash_groups, prehash_stats) =
            phase2_prehash(size_groups, self.hasher.clone(), prehash_config);

        summary.eliminated_by_prehash = prehash_stats.unique_prehashes;
        summary.cache_prehash_hits = prehash_stats.cache_hits;
        summary.cache_prehash_misses = prehash_stats.cache_misses;

        if !prehash_stats.errors.is_empty() {
            if self.config.strict {
                return Err(FinderError::ScanError(
                    crate::scanner::ScanError::HashError(prehash_stats.errors[0].clone()),
                ));
            } else {
                summary.scan_errors.extend(
                    prehash_stats
                        .errors
                        .into_iter()
                        .map(crate::scanner::ScanError::from),
                );
            }
        }

        if prehash_stats.interrupted || self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if prehash_groups.is_empty() {
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 3: Full hash comparison
        let fullhash_config = FullhashConfig {
            io_threads: self.config.io_threads,
            cache: self.config.cache.clone(),
            shutdown_flag: self.config.shutdown_flag.clone(),
            progress_callback: self.config.progress_callback.clone(),
            reference_paths: self.config.reference_paths.clone(),
        };

        let (duplicate_groups, fullhash_stats) =
            phase3_fullhash(prehash_groups, self.hasher.clone(), fullhash_config);

        if !fullhash_stats.errors.is_empty() {
            if self.config.strict {
                return Err(FinderError::ScanError(
                    crate::scanner::ScanError::HashError(fullhash_stats.errors[0].clone()),
                ));
            } else {
                summary.scan_errors.extend(
                    fullhash_stats
                        .errors
                        .into_iter()
                        .map(crate::scanner::ScanError::from),
                );
            }
        }

        if fullhash_stats.interrupted || self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        // Update summary
        summary.duplicate_groups = fullhash_stats.duplicate_groups;
        summary.duplicate_files = fullhash_stats.duplicate_files;
        summary.reclaimable_space = fullhash_stats.wasted_space;
        summary.cache_fullhash_hits = fullhash_stats.cache_hits;
        summary.cache_fullhash_misses = fullhash_stats.cache_misses;
        summary.scan_duration = start_time.elapsed();

        Ok((duplicate_groups, summary))
    }

    /// Find all duplicate files across multiple directories.
    ///
    /// Scans all provided paths using [`MultiWalker`] for parallel multi-directory
    /// traversal with path overlap detection. This prevents double-scanning when
    /// one path is nested within another.
    ///
    /// # Arguments
    ///
    /// * `paths` - Root directories to scan for duplicates
    ///
    /// # Returns
    ///
    /// A tuple of:
    /// - `Vec<DuplicateGroup>` - Confirmed duplicate groups
    /// - `ScanSummary` - Statistics about the scan
    ///
    /// # Errors
    ///
    /// Returns `FinderError` if:
    /// - All paths are invalid (non-existent or not directories)
    /// - The scan is interrupted by shutdown signal
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
    /// use std::path::PathBuf;
    ///
    /// let paths = vec![
    ///     PathBuf::from("/home/user/Documents"),
    ///     PathBuf::from("/home/user/Downloads"),
    /// ];
    ///
    /// let finder = DuplicateFinder::with_defaults();
    /// match finder.find_duplicates_in_paths(paths) {
    ///     Ok((groups, summary)) => {
    ///         println!("Found {} duplicate groups across directories", groups.len());
    ///         println!("Can reclaim {} bytes", summary.reclaimable_space);
    ///     }
    ///     Err(e) => eprintln!("Scan failed: {}", e),
    /// }
    /// ```
    ///
    /// [`MultiWalker`]: crate::scanner::MultiWalker
    pub fn find_duplicates_in_paths(
        &self,
        paths: Vec<PathBuf>,
    ) -> Result<(Vec<super::DuplicateGroup>, ScanSummary), FinderError> {
        let start_time = std::time::Instant::now();
        let mut summary = ScanSummary::default();

        // Handle empty paths
        if paths.is_empty() {
            log::warn!("No paths provided for scanning");
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Check for early shutdown
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        log::info!("Starting multi-directory scan of {} path(s)", paths.len());

        // Phase 0: Walk all directories and collect files
        if let Some(ref callback) = self.config.progress_callback {
            callback.on_phase_start("walking", 0);
            callback.on_message(&format!("Walking {} directories", paths.len()));
        }

        let mut multi_walker =
            crate::scanner::MultiWalker::new(paths, self.config.walker_config.clone());

        // Log the actual roots being scanned (after dedup/overlap detection)
        let roots = multi_walker.roots();
        if roots.is_empty() {
            log::warn!("No valid directories to scan after path normalization");
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        log::info!(
            "Scanning {} directory root(s): {:?}",
            roots.len(),
            roots.iter().map(|p| p.display()).collect::<Vec<_>>()
        );

        // Set shutdown flag on multi_walker if available
        if let Some(ref flag) = self.config.shutdown_flag {
            multi_walker = multi_walker.with_shutdown_flag(flag.clone());
        }

        // Set group map for named directory groups
        if !self.config.group_map.is_empty() {
            multi_walker = multi_walker.with_group_map(self.config.group_map.clone());
        }

        // Set progress callback on multi_walker if available
        if let Some(ref callback) = self.config.progress_callback {
            multi_walker = multi_walker.with_progress_callback(callback.clone());
        }

        let mut files = Vec::new();
        let mut seen_sizes = GrowableBloom::new(self.config.bloom_fp_rate, 1000);
        let mut duplicate_sizes = GrowableBloom::new(self.config.bloom_fp_rate, 1000);
        let mut first_occurrences: HashMap<u64, FileEntry> = HashMap::new();

        for result in multi_walker.walk() {
            match result {
                Ok(file) => {
                    // Empty files are handled separately by group_by_size
                    if file.size == 0 {
                        files.push(file);
                        continue;
                    }

                    if duplicate_sizes.contains(file.size) {
                        files.push(file);
                    } else if seen_sizes.contains(file.size) {
                        duplicate_sizes.insert(file.size);
                        if let Some(first) = first_occurrences.remove(&file.size) {
                            files.push(first);
                        }
                        files.push(file);
                    } else {
                        seen_sizes.insert(file.size);
                        first_occurrences.insert(file.size, file);
                    }
                }
                Err(e) => {
                    if self.config.strict {
                        return Err(FinderError::ScanError(e));
                    } else {
                        summary.scan_errors.push(e);
                    }
                }
            }
        }

        if let Some(ref callback) = self.config.progress_callback {
            callback.on_phase_end("walking");
        }

        summary.total_files = files.len() + first_occurrences.len();
        summary.total_size = files.iter().map(|f| f.size).sum::<u64>()
            + first_occurrences.values().map(|f| f.size).sum::<u64>();

        log::info!(
            "Found {} files ({} total) across all directories",
            summary.total_files,
            format_size(summary.total_size)
        );

        // Check for shutdown after walking
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if files.is_empty() {
            log::info!("No potential duplicates found across all directories, scan complete");
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Continue with the duplicate detection pipeline (same as find_duplicates_from_files)
        // Phase 1: Group by size
        log::info!("Phase 1: Grouping by size...");
        let (size_groups, size_stats) = super::group_by_size(files);

        // Update eliminated count to include files we discarded during walk
        summary.eliminated_by_size = size_stats.eliminated_unique + first_occurrences.len();

        log::info!(
            "Phase 1 complete: {} → {} files ({:.1}% eliminated)",
            size_stats.total_files,
            size_stats.potential_duplicates,
            size_stats.elimination_rate()
        );

        // Check for shutdown after Phase 1
        if self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if size_groups.is_empty() {
            log::info!("No potential duplicates found after size grouping");
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 2: Prehash comparison
        log::info!("Phase 2: Computing prehashes...");
        let prehash_config = PrehashConfig {
            io_threads: self.config.io_threads,
            cache: self.config.cache.clone(),
            shutdown_flag: self.config.shutdown_flag.clone(),
            progress_callback: self.config.progress_callback.clone(),
            reference_paths: self.config.reference_paths.clone(),
            bloom_fp_rate: self.config.bloom_fp_rate,
        };

        let (prehash_groups, prehash_stats) =
            phase2_prehash(size_groups, self.hasher.clone(), prehash_config);

        summary.eliminated_by_prehash = prehash_stats.unique_prehashes;
        summary.cache_prehash_hits = prehash_stats.cache_hits;
        summary.cache_prehash_misses = prehash_stats.cache_misses;

        if !prehash_stats.errors.is_empty() {
            if self.config.strict {
                return Err(FinderError::ScanError(
                    crate::scanner::ScanError::HashError(prehash_stats.errors[0].clone()),
                ));
            } else {
                summary.scan_errors.extend(
                    prehash_stats
                        .errors
                        .into_iter()
                        .map(crate::scanner::ScanError::from),
                );
            }
        }

        if prehash_stats.interrupted || self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        if prehash_groups.is_empty() {
            summary.scan_duration = start_time.elapsed();
            return Ok((Vec::new(), summary));
        }

        // Phase 3: Full hash comparison
        let fullhash_config = FullhashConfig {
            io_threads: self.config.io_threads,
            cache: self.config.cache.clone(),
            shutdown_flag: self.config.shutdown_flag.clone(),
            progress_callback: self.config.progress_callback.clone(),
            reference_paths: self.config.reference_paths.clone(),
        };

        let (duplicate_groups, fullhash_stats) =
            phase3_fullhash(prehash_groups, self.hasher.clone(), fullhash_config);

        if !fullhash_stats.errors.is_empty() {
            if self.config.strict {
                return Err(FinderError::ScanError(
                    crate::scanner::ScanError::HashError(fullhash_stats.errors[0].clone()),
                ));
            } else {
                summary.scan_errors.extend(
                    fullhash_stats
                        .errors
                        .into_iter()
                        .map(crate::scanner::ScanError::from),
                );
            }
        }

        if fullhash_stats.interrupted || self.config.is_shutdown_requested() {
            return Err(FinderError::Interrupted);
        }

        // Update summary
        summary.duplicate_groups = fullhash_stats.duplicate_groups;
        summary.duplicate_files = fullhash_stats.duplicate_files;
        summary.reclaimable_space = fullhash_stats.wasted_space;
        summary.cache_fullhash_hits = fullhash_stats.cache_hits;
        summary.cache_fullhash_misses = fullhash_stats.cache_misses;
        summary.scan_duration = start_time.elapsed();

        log::info!(
            "Multi-directory scan complete: {} duplicate groups, {} duplicate files, {} reclaimable",
            summary.duplicate_groups,
            summary.duplicate_files,
            summary.reclaimable_display()
        );

        Ok((duplicate_groups, summary))
    }
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
            errors: Vec::new(),
            cache_hits: 0,
            cache_misses: 100,
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

    // Phase 3 tests

    #[test]
    fn test_fullhash_config_default() {
        let config = FullhashConfig::default();
        assert_eq!(config.io_threads, 4);
        assert!(config.shutdown_flag.is_none());
        assert!(config.progress_callback.is_none());
    }

    #[test]
    fn test_fullhash_config_builder() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let config = FullhashConfig::default()
            .with_io_threads(8)
            .with_shutdown_flag(shutdown.clone());

        assert_eq!(config.io_threads, 8);
        assert!(config.shutdown_flag.is_some());
    }

    #[test]
    fn test_fullhash_stats_default() {
        let stats = FullhashStats::default();
        assert_eq!(stats.input_files, 0);
        assert_eq!(stats.hashed_files, 0);
        assert_eq!(stats.bytes_hashed, 0);
        assert_eq!(stats.duplicate_groups, 0);
    }

    #[test]
    fn test_phase3_empty_input() {
        let hasher = Arc::new(Hasher::new());
        let config = FullhashConfig::default();
        let (groups, stats) = phase3_fullhash(HashMap::new(), hasher, config);

        assert!(groups.is_empty());
        assert_eq!(stats.input_files, 0);
        assert_eq!(stats.duplicate_groups, 0);
    }

    #[test]
    fn test_phase3_identical_files() {
        let dir = TempDir::new().unwrap();
        let content = b"identical content for testing duplicates";

        let file1 = create_test_file(&dir, "file1.txt", content);
        let file2 = create_test_file(&dir, "file2.txt", content);

        // Create prehash groups (simulating Phase 2 output)
        let hasher = Arc::new(Hasher::new());
        let prehash = hasher.prehash(&file1.path).unwrap();

        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(prehash, vec![file1, file2]);

        let config = FullhashConfig::default();
        let (groups, stats) = phase3_fullhash(prehash_groups, hasher, config);

        // Both files should be in same duplicate group
        assert_eq!(groups.len(), 1);
        assert_eq!(stats.input_files, 2);
        assert_eq!(stats.hashed_files, 2);
        assert_eq!(stats.duplicate_groups, 1);
        assert_eq!(stats.duplicate_files, 1); // 1 duplicate (2 files - 1 original)
        assert_eq!(stats.wasted_space, content.len() as u64);
    }

    #[test]
    fn test_phase3_different_content_same_prehash_size() {
        let dir = TempDir::new().unwrap();

        // Files with same size but different content (hypothetically same prehash)
        // In reality, different content means different prehash, but for testing
        // we simulate false positives from Phase 2
        let file1 = create_test_file(&dir, "file1.txt", b"content A for test");
        let file2 = create_test_file(&dir, "file2.txt", b"content B for test");

        // Force them into same prehash group (simulating edge case)
        let fake_prehash = [0u8; 32];
        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(fake_prehash, vec![file1, file2]);

        let hasher = Arc::new(Hasher::new());
        let config = FullhashConfig::default();
        let (groups, stats) = phase3_fullhash(prehash_groups, hasher, config);

        // Files have different full hashes, no duplicates
        assert!(groups.is_empty());
        assert_eq!(stats.input_files, 2);
        assert_eq!(stats.hashed_files, 2);
        assert_eq!(stats.duplicate_groups, 0);
    }

    #[test]
    fn test_phase3_handles_missing_file() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "exists.txt", b"real content here");

        // Create entry for non-existent file
        let file2 = make_file_entry(dir.path().join("missing.txt").to_str().unwrap(), 17);

        let fake_prehash = [0u8; 32];
        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(fake_prehash, vec![file1, file2]);

        let hasher = Arc::new(Hasher::new());
        let config = FullhashConfig::default();
        let (groups, stats) = phase3_fullhash(prehash_groups, hasher, config);

        // Missing file should fail, existing file becomes unique (no group)
        assert!(groups.is_empty());
        assert_eq!(stats.input_files, 2);
        assert_eq!(stats.hashed_files, 1);
        assert_eq!(stats.failed_files, 1);
    }

    #[test]
    fn test_phase3_shutdown_flag() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "file1.txt", b"content");
        let file2 = create_test_file(&dir, "file2.txt", b"content");

        let fake_prehash = [0u8; 32];
        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(fake_prehash, vec![file1, file2]);

        let shutdown = Arc::new(AtomicBool::new(true)); // Already shutdown
        let hasher = Arc::new(Hasher::new());
        let config = FullhashConfig::default().with_shutdown_flag(shutdown);
        let (_, stats) = phase3_fullhash(prehash_groups, hasher, config);

        // Should be interrupted
        assert!(stats.interrupted);
    }

    #[test]
    fn test_phase3_multiple_duplicate_groups() {
        let dir = TempDir::new().unwrap();

        // Group 1: Two identical files
        let file1 = create_test_file(&dir, "a1.txt", b"content group A");
        let file2 = create_test_file(&dir, "a2.txt", b"content group A");

        // Group 2: Three identical files
        let file3 = create_test_file(&dir, "b1.txt", b"content group B");
        let file4 = create_test_file(&dir, "b2.txt", b"content group B");
        let file5 = create_test_file(&dir, "b3.txt", b"content group B");

        let hasher = Arc::new(Hasher::new());
        let prehash1 = hasher.prehash(&file1.path).unwrap();
        let prehash2 = hasher.prehash(&file3.path).unwrap();

        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(prehash1, vec![file1, file2]);
        prehash_groups.insert(prehash2, vec![file3, file4, file5]);

        let config = FullhashConfig::default();
        let (groups, stats) = phase3_fullhash(prehash_groups, hasher, config);

        // Should have 2 duplicate groups
        assert_eq!(groups.len(), 2);
        assert_eq!(stats.input_files, 5);
        assert_eq!(stats.hashed_files, 5);
        assert_eq!(stats.duplicate_groups, 2);
        assert_eq!(stats.duplicate_files, 3); // 1 + 2
    }

    #[test]
    fn test_phase3_bytes_hashed_tracking() {
        let dir = TempDir::new().unwrap();
        let content = b"test content for byte tracking";
        let file1 = create_test_file(&dir, "file1.txt", content);
        let file2 = create_test_file(&dir, "file2.txt", content);

        let hasher = Arc::new(Hasher::new());
        let prehash = hasher.prehash(&file1.path).unwrap();

        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(prehash, vec![file1, file2]);

        let config = FullhashConfig::default();
        let (_, stats) = phase3_fullhash(prehash_groups, hasher, config);

        // Should track total bytes hashed
        assert_eq!(stats.bytes_hashed, (content.len() * 2) as u64);
    }

    #[test]
    fn test_phase3_progress_callback() {
        let dir = TempDir::new().unwrap();
        let file1 = create_test_file(&dir, "file1.txt", b"content");
        let file2 = create_test_file(&dir, "file2.txt", b"content");

        let hasher = Arc::new(Hasher::new());
        let prehash = hasher.prehash(&file1.path).unwrap();

        let mut prehash_groups = HashMap::new();
        prehash_groups.insert(prehash, vec![file1, file2]);

        let callback = Arc::new(TestProgressCallback::new());
        let config = FullhashConfig::default().with_progress_callback(callback.clone());

        let (_, _) = phase3_fullhash(prehash_groups, hasher, config);

        // Callback should have been called
        assert!(*callback.phase_started.lock().unwrap());
        assert!(callback.progress_count.load(Ordering::SeqCst) > 0);
        assert!(*callback.phase_ended.lock().unwrap());
    }

    // ========================================================================
    // DuplicateFinder Tests
    // ========================================================================

    #[test]
    fn test_finder_config_default() {
        let config = FinderConfig::default();
        assert_eq!(config.io_threads, 4);
        assert!(!config.paranoid);
        assert!(config.shutdown_flag.is_none());
        assert!(config.progress_callback.is_none());
    }

    #[test]
    fn test_finder_config_builder() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let config = FinderConfig::default()
            .with_io_threads(8)
            .with_paranoid(true)
            .with_shutdown_flag(shutdown.clone());

        assert_eq!(config.io_threads, 8);
        assert!(config.paranoid);
        assert!(config.shutdown_flag.is_some());
    }

    #[test]
    fn test_finder_config_io_threads_min() {
        let config = FinderConfig::default().with_io_threads(0);
        assert_eq!(config.io_threads, 1); // Minimum 1
    }

    #[test]
    fn test_scan_summary_default() {
        let summary = ScanSummary::default();
        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.total_size, 0);
        assert_eq!(summary.duplicate_groups, 0);
        assert_eq!(summary.reclaimable_space, 0);
        assert!(!summary.interrupted);
    }

    #[test]
    fn test_scan_summary_wasted_percentage() {
        let summary = ScanSummary {
            total_size: 1000,
            reclaimable_space: 250,
            ..Default::default()
        };
        assert!((summary.wasted_percentage() - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_scan_summary_wasted_percentage_zero_size() {
        let summary = ScanSummary::default();
        assert_eq!(summary.wasted_percentage(), 0.0);
    }

    #[test]
    fn test_scan_summary_display() {
        let summary = ScanSummary {
            total_size: 1_500_000,
            reclaimable_space: 500_000,
            ..Default::default()
        };
        assert!(summary.total_size_display().contains("MB"));
        assert!(summary.reclaimable_display().contains("KB"));
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert!(format_size(1024).contains("KB"));
        assert!(format_size(2048).contains("KB"));
    }

    #[test]
    fn test_format_size_megabytes() {
        assert!(format_size(1024 * 1024).contains("MB"));
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert!(format_size(1024 * 1024 * 1024).contains("GB"));
    }

    #[test]
    fn test_format_size_terabytes() {
        assert!(format_size(1024 * 1024 * 1024 * 1024).contains("TB"));
    }

    #[test]
    fn test_duplicate_finder_new() {
        let config = FinderConfig::default();
        let finder = DuplicateFinder::new(config);
        // Just ensure it creates successfully
        assert!(Arc::strong_count(&finder.hasher) >= 1);
    }

    #[test]
    fn test_duplicate_finder_with_defaults() {
        let finder = DuplicateFinder::with_defaults();
        assert!(Arc::strong_count(&finder.hasher) >= 1);
    }

    #[test]
    fn test_find_duplicates_path_not_found() {
        let finder = DuplicateFinder::with_defaults();
        let result = finder.find_duplicates(std::path::Path::new("/nonexistent/path/12345"));

        assert!(result.is_err());
        match result.unwrap_err() {
            FinderError::PathNotFound(p) => {
                assert!(p.to_string_lossy().contains("nonexistent"));
            }
            _ => panic!("Expected PathNotFound error"),
        }
    }

    #[test]
    fn test_find_duplicates_not_a_directory() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let finder = DuplicateFinder::with_defaults();
        let result = finder.find_duplicates(&file);

        assert!(result.is_err());
        match result.unwrap_err() {
            FinderError::NotADirectory(_) => {}
            _ => panic!("Expected NotADirectory error"),
        }
    }

    #[test]
    fn test_find_duplicates_empty_directory() {
        let dir = TempDir::new().unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

        assert!(groups.is_empty());
        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.duplicate_groups, 0);
        assert!(!summary.interrupted);
    }

    #[test]
    fn test_find_duplicates_no_duplicates() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "content a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "content b").unwrap();
        std::fs::write(dir.path().join("c.txt"), "content c").unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

        assert!(groups.is_empty());
        assert_eq!(summary.total_files, 3);
        assert_eq!(summary.duplicate_groups, 0);
        assert_eq!(summary.reclaimable_space, 0);
    }

    #[test]
    fn test_find_duplicates_with_duplicates() {
        let dir = TempDir::new().unwrap();
        let content = b"identical content for duplicate detection";
        std::fs::write(dir.path().join("dup1.txt"), content).unwrap();
        std::fs::write(dir.path().join("dup2.txt"), content).unwrap();
        std::fs::write(dir.path().join("unique.txt"), "different content").unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

        assert_eq!(groups.len(), 1);
        assert_eq!(summary.total_files, 3);
        assert_eq!(summary.duplicate_groups, 1);
        assert_eq!(summary.duplicate_files, 1); // 2 files - 1 original
        assert_eq!(summary.reclaimable_space, content.len() as u64);
    }

    #[test]
    fn test_find_duplicates_multiple_groups() {
        let dir = TempDir::new().unwrap();

        // Group A: 2 identical files
        std::fs::write(dir.path().join("a1.txt"), "group A content").unwrap();
        std::fs::write(dir.path().join("a2.txt"), "group A content").unwrap();

        // Group B: 3 identical files
        std::fs::write(dir.path().join("b1.txt"), "group B content").unwrap();
        std::fs::write(dir.path().join("b2.txt"), "group B content").unwrap();
        std::fs::write(dir.path().join("b3.txt"), "group B content").unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

        assert_eq!(groups.len(), 2);
        assert_eq!(summary.total_files, 5);
        assert_eq!(summary.duplicate_groups, 2);
        assert_eq!(summary.duplicate_files, 3); // (2-1) + (3-1)
    }

    #[test]
    fn test_find_duplicates_shutdown_flag() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "content").unwrap();

        let shutdown = Arc::new(AtomicBool::new(true)); // Already shutdown
        let config = FinderConfig::default().with_shutdown_flag(shutdown);
        let finder = DuplicateFinder::new(config);

        let result = finder.find_duplicates(dir.path());

        assert!(result.is_err());
        match result.unwrap_err() {
            FinderError::Interrupted => {}
            _ => panic!("Expected Interrupted error"),
        }
    }

    #[test]
    fn test_find_duplicates_from_files_empty() {
        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder.find_duplicates_from_files(vec![]).unwrap();

        assert!(groups.is_empty());
        assert_eq!(summary.total_files, 0);
    }

    #[test]
    fn test_find_duplicates_from_files_with_duplicates() {
        let dir = TempDir::new().unwrap();
        let content = b"identical content";

        let file1 = create_test_file(&dir, "dup1.txt", content);
        let file2 = create_test_file(&dir, "dup2.txt", content);

        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder
            .find_duplicates_from_files(vec![file1, file2])
            .unwrap();

        assert_eq!(groups.len(), 1);
        assert_eq!(summary.duplicate_groups, 1);
        assert_eq!(summary.reclaimable_space, content.len() as u64);
    }

    #[test]
    fn test_find_duplicates_summary_timing() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "content").unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (_, summary) = finder.find_duplicates(dir.path()).unwrap();

        // Duration should be non-zero (at least some time passed)
        assert!(summary.scan_duration.as_nanos() > 0);
    }

    #[test]
    fn test_find_duplicates_with_progress_callback() {
        let dir = TempDir::new().unwrap();
        let content = b"identical content for testing";
        std::fs::write(dir.path().join("dup1.txt"), content).unwrap();
        std::fs::write(dir.path().join("dup2.txt"), content).unwrap();

        let callback = Arc::new(TestProgressCallback::new());
        let config = FinderConfig::default().with_progress_callback(callback.clone());
        let finder = DuplicateFinder::new(config);

        let (groups, _) = finder.find_duplicates(dir.path()).unwrap();

        assert_eq!(groups.len(), 1);
        // Progress callback should have been invoked
        assert!(*callback.phase_started.lock().unwrap());
        assert!(*callback.phase_ended.lock().unwrap());
    }
}
