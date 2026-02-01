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
//! use std::path::Path;
//!
//! let finder = DuplicateFinder::with_defaults();
//! let (groups, summary) = finder.find_duplicates(Path::new(".")).unwrap();
//!
//! // Compact JSON
//! let output = JsonOutput::new(&groups, &summary);
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
                .map(|p| normalize_path(p.as_path()))
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
    /// Whether the scan was interrupted
    pub interrupted: bool,
}

impl JsonSummary {
    /// Create a JSON summary from a ScanSummary.
    #[must_use]
    pub fn from_scan_summary(summary: &ScanSummary) -> Self {
        Self {
            total_files: summary.total_files,
            total_size: summary.total_size,
            duplicate_groups: summary.duplicate_groups,
            duplicate_files: summary.duplicate_files,
            reclaimable_space: summary.reclaimable_space,
            scan_duration_ms: summary.scan_duration.as_millis() as u64,
            interrupted: summary.interrupted,
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
    /// Create a new JSON output from duplicate groups and summary.
    ///
    /// # Arguments
    ///
    /// * `groups` - The duplicate groups found during scanning
    /// * `summary` - The scan summary statistics
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::duplicates::{DuplicateGroup, ScanSummary};
    /// use rustdupe::output::json::JsonOutput;
    /// use std::path::PathBuf;
    ///
    /// let groups = vec![
    ///     DuplicateGroup::new([0u8; 32], 1024, vec![
    ///         PathBuf::from("/file1.txt"),
    ///         PathBuf::from("/file2.txt"),
    ///     ]),
    /// ];
    /// let summary = ScanSummary::default();
    ///
    /// let output = JsonOutput::new(&groups, &summary);
    /// assert_eq!(output.duplicates.len(), 1);
    /// ```
    #[must_use]
    pub fn new(groups: &[DuplicateGroup], summary: &ScanSummary) -> Self {
        Self {
            duplicates: groups
                .iter()
                .map(JsonDuplicateGroup::from_duplicate_group)
                .collect(),
            summary: JsonSummary::from_scan_summary(summary),
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
    ///
    /// let output = JsonOutput::new(&[], &ScanSummary::default());
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
    ///
    /// let output = JsonOutput::new(&[], &ScanSummary::default());
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
    #[error("I/O error: {0}")]
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
            interrupted: false,
        }
    }

    fn create_test_groups() -> Vec<DuplicateGroup> {
        vec![
            DuplicateGroup::new(
                [0u8; 32],
                1024,
                vec![
                    PathBuf::from("/path/to/file1.txt"),
                    PathBuf::from("/path/to/file2.txt"),
                ],
            ),
            DuplicateGroup::new(
                [1u8; 32],
                2048,
                vec![
                    PathBuf::from("/path/to/fileA.txt"),
                    PathBuf::from("/path/to/fileB.txt"),
                    PathBuf::from("/path/to/fileC.txt"),
                ],
            ),
        ]
    }

    #[test]
    fn test_json_output_empty() {
        let output = JsonOutput::new(&[], &ScanSummary::default());
        assert!(output.duplicates.is_empty());
        assert_eq!(output.summary.total_files, 0);
    }

    #[test]
    fn test_json_output_with_groups() {
        let groups = create_test_groups();
        let summary = create_test_summary();
        let output = JsonOutput::new(&groups, &summary);

        assert_eq!(output.duplicates.len(), 2);
        assert_eq!(output.duplicates[0].files.len(), 2);
        assert_eq!(output.duplicates[1].files.len(), 3);
        assert_eq!(output.summary.duplicate_groups, 5);
        assert_eq!(output.summary.scan_duration_ms, 1234);
    }

    #[test]
    fn test_to_json_compact() {
        let output = JsonOutput::new(&[], &ScanSummary::default());
        let json = output.to_json().unwrap();

        // Compact JSON should be a single line
        assert!(!json.contains('\n'));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn test_to_json_pretty() {
        let output = JsonOutput::new(&[], &ScanSummary::default());
        let json = output.to_json_pretty().unwrap();

        // Pretty JSON should have newlines
        assert!(json.contains('\n'));
        assert!(json.starts_with('{'));
    }

    #[test]
    fn test_json_is_valid() {
        let groups = create_test_groups();
        let summary = create_test_summary();
        let output = JsonOutput::new(&groups, &summary);
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
        let groups = vec![DuplicateGroup::new(
            [0xab; 32],
            1024,
            vec![PathBuf::from("/test.txt")],
        )];
        let output = JsonOutput::new(&groups, &ScanSummary::default());

        // Hash should be 64 hex characters
        assert_eq!(output.duplicates[0].hash.len(), 64);
        assert!(output.duplicates[0]
            .hash
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_write_to() {
        let output = JsonOutput::new(&[], &ScanSummary::default());
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
        let json_summary = JsonSummary::from_scan_summary(&summary);
        assert_eq!(json_summary.scan_duration_ms, 5000);
    }

    #[test]
    fn test_json_summary_interrupted() {
        let summary = ScanSummary {
            interrupted: true,
            ..Default::default()
        };
        let output = JsonOutput::new(&[], &summary);
        assert!(output.summary.interrupted);
    }
}
