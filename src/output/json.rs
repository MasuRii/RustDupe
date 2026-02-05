//! JSON output formatter for duplicate scan results.
//!
//! Provides machine-readable JSON output for scripting and automation.
//!
//! # Output Schema
//!
//! ```json
//! {
//!   "duplicates": [
//!     {
//!       "hash": "abc123...",
//!       "size": 1024,
//!       "files": ["/path/to/file1.txt", "/path/to/file2.txt"]
//!     }
//!   ],
//!   "summary": {
//!     "total_files": 100,
//!     "total_size": 1048576,
//!     "duplicate_groups": 5,
//!     "duplicate_files": 10,
//!     "reclaimable_space": 51200,
//!     "scan_duration_ms": 1234,
//!     "interrupted": false
//!   }
//! }
//! ```
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::duplicates::{DuplicateFinder, DuplicateGroup, ScanSummary};
//! use rustdupe::output::json::JsonOutput;
//! use rustdupe::error::ExitCode;
//! use std::path::Path;
//!
//! let finder = DuplicateFinder::with_defaults();
//! let (groups, summary) = finder.find_duplicates(Path::new(".")).unwrap();
//!
//! // Compact JSON
//! let output = JsonOutput::new(&groups, &summary, ExitCode::Success);
//! println!("{}", output.to_json().unwrap());
//!
//! // Pretty-printed JSON
//! println!("{}", output.to_json_pretty().unwrap());
//! ```

use std::io::Write;

use serde::Serialize;

use crate::duplicates::{DuplicateGroup, ScanSummary};

/// A single duplicate group in JSON format.
#[derive(Debug, Clone, Serialize)]
pub struct JsonDuplicateGroup {
    /// BLAKE3 hash as hexadecimal string (64 characters)
    pub hash: String,
    /// File size in bytes
    pub size: u64,
    /// Absolute paths to all duplicate files
    pub files: Vec<String>,
}

impl JsonDuplicateGroup {
    /// Create a JSON duplicate group from a DuplicateGroup.
    ///
    /// Paths are converted to absolute paths where possible.
    #[must_use]
    pub fn from_duplicate_group(group: &DuplicateGroup) -> Self {
        Self {
            hash: group.hash_hex(),
            size: group.size,
            files: group
                .files
                .iter()
                .map(|f| normalize_path(f.path.as_path()))
                .collect(),
        }
    }
}

/// Summary statistics in JSON format.
#[derive(Debug, Clone, Serialize)]
pub struct JsonSummary {
    /// Total number of files scanned
    pub total_files: usize,
    /// Total size of all scanned files in bytes
    pub total_size: u64,
    /// Number of confirmed duplicate groups
    pub duplicate_groups: usize,
    /// Total number of duplicate files (excluding originals)
    pub duplicate_files: usize,
    /// Total space that can be reclaimed by removing duplicates (bytes)
    pub reclaimable_space: u64,
    /// Duration of the scan in milliseconds
    pub scan_duration_ms: u64,
    /// Duration of the walking phase in milliseconds
    pub walk_duration_ms: u64,
    /// Duration of the perceptual hashing phase in milliseconds
    pub perceptual_duration_ms: u64,
    /// Duration of the size grouping phase in milliseconds
    pub size_duration_ms: u64,
    /// Duration of the prehash phase in milliseconds
    pub prehash_duration_ms: u64,
    /// Duration of the full hash phase in milliseconds
    pub fullhash_duration_ms: u64,
    /// Duration of the similar image detection phase in milliseconds
    pub clustering_duration_ms: u64,
    /// Whether the scan was interrupted
    pub interrupted: bool,
    /// The exit code number
    pub exit_code: i32,
    /// The machine-readable exit code name (e.g., "RD000")
    pub exit_code_name: String,
    /// Number of unique file sizes correctly identified by Bloom filter
    pub bloom_size_unique: usize,
    /// Number of unique file sizes incorrectly identified as duplicates by Bloom filter
    pub bloom_size_fp: usize,
    /// Observed false positive rate for the size Bloom filter (%)
    pub bloom_size_fp_rate: f64,
    /// Number of unique prehashes correctly identified by Bloom filter
    pub bloom_prehash_unique: usize,
    /// Number of unique prehashes incorrectly identified as duplicates by Bloom filter
    pub bloom_prehash_fp: usize,
    /// Observed false positive rate for the prehash Bloom filter (%)
    pub bloom_prehash_fp_rate: f64,
    /// Number of images processed for perceptual hashing
    pub images_perceptual_hashed: usize,
    /// Number of perceptual hash cache hits
    pub images_perceptual_hash_cache_hits: usize,
}

impl JsonSummary {
    /// Create a JSON summary from a ScanSummary and an exit code.
    #[must_use]
    pub fn from_scan_summary(summary: &ScanSummary, exit_code: crate::error::ExitCode) -> Self {
        Self {
            total_files: summary.total_files,
            total_size: summary.total_size,
            duplicate_groups: summary.duplicate_groups,
            duplicate_files: summary.duplicate_files,
            reclaimable_space: summary.reclaimable_space,
            scan_duration_ms: summary.scan_duration.as_millis() as u64,
            walk_duration_ms: summary.walk_duration.as_millis() as u64,
            perceptual_duration_ms: summary.perceptual_duration.as_millis() as u64,
            size_duration_ms: summary.size_duration.as_millis() as u64,
            prehash_duration_ms: summary.prehash_duration.as_millis() as u64,
            fullhash_duration_ms: summary.fullhash_duration.as_millis() as u64,
            clustering_duration_ms: summary.clustering_duration.as_millis() as u64,
            interrupted: summary.interrupted,
            exit_code: exit_code.as_i32(),
            exit_code_name: exit_code.code_prefix().to_string(),
            bloom_size_unique: summary.bloom_size_unique,
            bloom_size_fp: summary.bloom_size_fp,
            bloom_size_fp_rate: summary.bloom_size_fp_rate(),
            bloom_prehash_unique: summary.bloom_prehash_unique,
            bloom_prehash_fp: summary.bloom_prehash_fp,
            bloom_prehash_fp_rate: summary.bloom_prehash_fp_rate(),
            images_perceptual_hashed: summary.images_perceptual_hashed,
            images_perceptual_hash_cache_hits: summary.images_perceptual_hash_cache_hits,
        }
    }
}

/// Complete JSON output structure.
#[derive(Debug, Clone, Serialize)]
pub struct JsonOutput {
    /// List of duplicate groups
    pub duplicates: Vec<JsonDuplicateGroup>,
    /// Scan summary statistics
    pub summary: JsonSummary,
}

impl JsonOutput {
    /// Create a new JSON output from duplicate groups, summary and exit code.
    ///
    /// # Arguments
    ///
    /// * `groups` - The duplicate groups found during scanning
    /// * `summary` - The scan summary statistics
    /// * `exit_code` - The exit code for this run
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::duplicates::{DuplicateGroup, ScanSummary};
    /// use rustdupe::output::json::JsonOutput;
    /// use rustdupe::error::ExitCode;
    /// use std::path::PathBuf;
    ///
    /// let groups = vec![
    ///     DuplicateGroup::new([0u8; 32], 1024, vec![
    ///         rustdupe::scanner::FileEntry::new(PathBuf::from("/file1.txt"), 1024, std::time::SystemTime::now()),
    ///         rustdupe::scanner::FileEntry::new(PathBuf::from("/file2.txt"), 1024, std::time::SystemTime::now()),
    ///     ], Vec::new()),
    /// ];
    /// let summary = ScanSummary::default();
    ///
    /// let output = JsonOutput::new(&groups, &summary, ExitCode::Success);
    /// assert_eq!(output.duplicates.len(), 1);
    /// ```
    #[must_use]
    pub fn new(
        groups: &[DuplicateGroup],
        summary: &ScanSummary,
        exit_code: crate::error::ExitCode,
    ) -> Self {
        Self {
            duplicates: groups
                .iter()
                .map(JsonDuplicateGroup::from_duplicate_group)
                .collect(),
            summary: JsonSummary::from_scan_summary(summary, exit_code),
        }
    }

    /// Serialize to compact JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails (unlikely for valid data).
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::duplicates::{DuplicateGroup, ScanSummary};
    /// use rustdupe::output::json::JsonOutput;
    /// use rustdupe::error::ExitCode;
    ///
    /// let output = JsonOutput::new(&[], &ScanSummary::default(), ExitCode::Success);
    /// let json = output.to_json().unwrap();
    /// assert!(json.starts_with("{"));
    /// ```
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails (unlikely for valid data).
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::duplicates::{DuplicateGroup, ScanSummary};
    /// use rustdupe::output::json::JsonOutput;
    /// use rustdupe::error::ExitCode;
    ///
    /// let output = JsonOutput::new(&[], &ScanSummary::default(), ExitCode::Success);
    /// let json = output.to_json_pretty().unwrap();
    /// assert!(json.contains('\n'));  // Pretty-printed has newlines
    /// ```
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Write JSON to a writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - The writer to output to (e.g., stdout)
    /// * `pretty` - Whether to pretty-print the output
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn write_to<W: Write>(&self, writer: &mut W, pretty: bool) -> Result<(), JsonOutputError> {
        let json = if pretty {
            self.to_json_pretty()?
        } else {
            self.to_json()?
        };
        writer.write_all(json.as_bytes())?;
        writer.write_all(b"\n")?;
        Ok(())
    }
}

/// Normalize a path to an absolute path string.
///
/// Attempts to canonicalize the path. If that fails (e.g., file no longer exists),
/// falls back to the display representation.
fn normalize_path(path: &std::path::Path) -> String {
    match path.canonicalize() {
        Ok(canonical) => canonical.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// Errors that can occur during JSON output.
#[derive(thiserror::Error, Debug)]
pub enum JsonOutputError {
    /// JSON serialization error
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O error during writing
    #[error("I/O error during JSON generation: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn create_test_summary() -> ScanSummary {
        ScanSummary {
            total_files: 100,
            total_size: 1024 * 1024,
            eliminated_by_size: 50,
            eliminated_by_prehash: 30,
            cache_prehash_hits: 0,
            cache_prehash_misses: 0,
            cache_fullhash_hits: 0,
            cache_fullhash_misses: 0,
            duplicate_groups: 5,
            duplicate_files: 10,
            reclaimable_space: 51200,
            scan_duration: Duration::from_millis(1234),
            walk_duration: Duration::from_millis(100),
            perceptual_duration: Duration::from_millis(0),
            size_duration: Duration::from_millis(50),
            prehash_duration: Duration::from_millis(200),
            fullhash_duration: Duration::from_millis(800),
            clustering_duration: Duration::from_millis(0),
            interrupted: false,
            scan_errors: Vec::new(),
            bloom_size_unique: 45,
            bloom_size_fp: 5,
            bloom_prehash_unique: 25,
            bloom_prehash_fp: 5,
            images_perceptual_hashed: 0,
            images_perceptual_hash_cache_hits: 0,
        }
    }

    fn create_test_groups() -> Vec<DuplicateGroup> {
        let now = std::time::SystemTime::now();
        vec![
            DuplicateGroup::new(
                [0u8; 32],
                1024,
                vec![
                    crate::scanner::FileEntry::new(PathBuf::from("/path/to/file1.txt"), 1024, now),
                    crate::scanner::FileEntry::new(PathBuf::from("/path/to/file2.txt"), 1024, now),
                ],
                Vec::new(),
            ),
            DuplicateGroup::new(
                [1u8; 32],
                2048,
                vec![
                    crate::scanner::FileEntry::new(PathBuf::from("/path/to/fileA.txt"), 2048, now),
                    crate::scanner::FileEntry::new(PathBuf::from("/path/to/fileB.txt"), 2048, now),
                    crate::scanner::FileEntry::new(PathBuf::from("/path/to/fileC.txt"), 2048, now),
                ],
                Vec::new(),
            ),
        ]
    }

    #[test]
    fn test_json_output_empty() {
        let output = JsonOutput::new(
            &[],
            &ScanSummary::default(),
            crate::error::ExitCode::Success,
        );
        assert!(output.duplicates.is_empty());
        assert_eq!(output.summary.total_files, 0);
    }

    #[test]
    fn test_json_output_with_groups() {
        let groups = create_test_groups();
        let summary = create_test_summary();
        let output = JsonOutput::new(&groups, &summary, crate::error::ExitCode::Success);

        assert_eq!(output.duplicates.len(), 2);
        assert_eq!(output.duplicates[0].files.len(), 2);
        assert_eq!(output.duplicates[1].files.len(), 3);
        assert_eq!(output.summary.duplicate_groups, 5);
        assert_eq!(output.summary.scan_duration_ms, 1234);
    }

    #[test]
    fn test_to_json_compact() {
        let output = JsonOutput::new(
            &[],
            &ScanSummary::default(),
            crate::error::ExitCode::Success,
        );
        let json = output.to_json().unwrap();

        // Compact JSON should be a single line
        assert!(!json.contains('\n'));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn test_to_json_pretty() {
        let output = JsonOutput::new(
            &[],
            &ScanSummary::default(),
            crate::error::ExitCode::Success,
        );
        let json = output.to_json_pretty().unwrap();

        // Pretty JSON should have newlines
        assert!(json.contains('\n'));
        assert!(json.starts_with('{'));
    }

    #[test]
    fn test_json_is_valid() {
        let groups = create_test_groups();
        let summary = create_test_summary();
        let output = JsonOutput::new(&groups, &summary, crate::error::ExitCode::Success);
        let json = output.to_json().unwrap();

        // Parse it back to verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("duplicates").is_some());
        assert!(parsed.get("summary").is_some());

        let duplicates = parsed.get("duplicates").unwrap().as_array().unwrap();
        assert_eq!(duplicates.len(), 2);

        let summary = parsed.get("summary").unwrap();
        assert_eq!(summary.get("total_files").unwrap().as_u64().unwrap(), 100);
    }

    #[test]
    fn test_hash_format() {
        let now = std::time::SystemTime::now();
        let groups = vec![DuplicateGroup::new(
            [0xab; 32],
            1024,
            vec![crate::scanner::FileEntry::new(
                PathBuf::from("/test.txt"),
                1024,
                now,
            )],
            Vec::new(),
        )];
        let output = JsonOutput::new(
            &groups,
            &ScanSummary::default(),
            crate::error::ExitCode::Success,
        );

        // Hash should be 64 hex characters
        assert_eq!(output.duplicates[0].hash.len(), 64);
        assert!(output.duplicates[0]
            .hash
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_write_to() {
        let output = JsonOutput::new(
            &[],
            &ScanSummary::default(),
            crate::error::ExitCode::Success,
        );
        let mut buffer = Vec::new();

        output.write_to(&mut buffer, false).unwrap();

        let written = String::from_utf8(buffer).unwrap();
        assert!(written.starts_with('{'));
        assert!(written.ends_with("}\n"));
    }

    #[test]
    fn test_json_summary_duration() {
        let summary = ScanSummary {
            scan_duration: Duration::from_secs(5),
            ..Default::default()
        };
        let json_summary =
            JsonSummary::from_scan_summary(&summary, crate::error::ExitCode::Success);
        assert_eq!(json_summary.scan_duration_ms, 5000);
    }

    #[test]
    fn test_json_summary_interrupted() {
        let summary = ScanSummary {
            interrupted: true,
            ..Default::default()
        };
        let output = JsonOutput::new(&[], &summary, crate::error::ExitCode::Interrupted);
        assert!(output.summary.interrupted);
        assert_eq!(output.summary.exit_code, 130);
    }
}
