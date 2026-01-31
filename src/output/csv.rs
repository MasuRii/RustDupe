//! CSV output formatter for duplicate scan results.
//!
//! Provides machine-readable CSV output for spreadsheets and data analysis.
//! One row is generated for each duplicate file.
//!
//! # Columns
//!
//! - `group_id`: Numeric ID identifying the duplicate group
//! - `hash`: BLAKE3 content hash (hexadecimal)
//! - `path`: Absolute path to the file
//! - `size`: File size in bytes
//! - `modified`: Last modified time (RFC 3339 format)
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::duplicates::{DuplicateFinder, DuplicateGroup};
//! use rustdupe::output::csv::CsvOutput;
//! use std::path::Path;
//!
//! let finder = DuplicateFinder::with_defaults();
//! let (groups, _) = finder.find_duplicates(Path::new(".")).unwrap();
//!
//! let output = CsvOutput::new(&groups);
//! output.write_to(std::io::stdout()).unwrap();
//! ```

use std::io;

use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;

use crate::duplicates::DuplicateGroup;

/// Errors that can occur during CSV output generation.
#[derive(Debug, Error)]
pub enum CsvOutputError {
    /// I/O error during writing.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Error during CSV serialization.
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
}

/// A single row in the CSV output.
#[derive(Debug, Serialize)]
struct CsvRow {
    /// Unique identifier for the duplicate group
    group_id: usize,
    /// BLAKE3 hash of the file content (hex)
    hash: String,
    /// Absolute path to the file
    path: String,
    /// File size in bytes
    size: u64,
    /// Last modified time (RFC 3339)
    modified: String,
}

/// CSV output formatter.
pub struct CsvOutput<'a> {
    groups: &'a [DuplicateGroup],
}

impl<'a> CsvOutput<'a> {
    /// Create a new CSV output formatter.
    #[must_use]
    pub fn new(groups: &'a [DuplicateGroup]) -> Self {
        Self { groups }
    }

    /// Write the CSV output to the given writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - The writer to output to
    ///
    /// # Errors
    ///
    /// Returns `CsvOutputError` if writing or serialization fails.
    pub fn write_to<W: io::Write>(&self, writer: W) -> Result<(), CsvOutputError> {
        let mut csv_writer = csv::Writer::from_writer(writer);

        for (idx, group) in self.groups.iter().enumerate() {
            let group_id = idx + 1;
            let hash_hex = group.hash_hex();

            for path in &group.files {
                let modified = get_modified_time(path);

                let row = CsvRow {
                    group_id,
                    hash: hash_hex.clone(),
                    path: path.to_string_lossy().to_string(),
                    size: group.size,
                    modified,
                };

                csv_writer.serialize(row)?;
            }
        }

        csv_writer.flush()?;
        Ok(())
    }

    /// Generate CSV output as a string.
    ///
    /// # Errors
    ///
    /// Returns `CsvOutputError` if serialization fails.
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::output::csv::CsvOutput;
    /// let output = CsvOutput::new(&[]);
    /// let csv = output.to_string().unwrap();
    /// ```
    pub fn to_string(&self) -> Result<String, CsvOutputError> {
        let mut buffer = Vec::new();
        self.write_to(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }
}

/// Helper function to get formatted modified time for a file.
/// Falls back to "unknown" if metadata cannot be read.
fn get_modified_time(path: &std::path::Path) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|m| {
            let datetime: DateTime<Utc> = m.into();
            datetime.to_rfc3339()
        })
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_csv_output_basic() {
        let dir = TempDir::new().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        File::create(&file1).unwrap().write_all(b"content").unwrap();
        File::create(&file2).unwrap().write_all(b"content").unwrap();

        let groups = vec![DuplicateGroup::new(
            [0u8; 32],
            7,
            vec![file1.clone(), file2.clone()],
        )];

        let output = CsvOutput::new(&groups);
        let csv_str = output.to_string().unwrap();

        // Check header
        assert!(csv_str.contains("group_id,hash,path,size,modified"));
        // Check rows (very basic check)
        assert!(
            csv_str.contains("1,0000000000000000000000000000000000000000000000000000000000000000")
        );
        assert!(csv_str.contains("file1.txt"));
        assert!(csv_str.contains("file2.txt"));
        assert!(csv_str.contains(",7,"));
    }

    #[test]
    fn test_csv_output_quoting() {
        let dir = TempDir::new().unwrap();
        let file_with_comma = dir.path().join("file,with,comma.txt");
        File::create(&file_with_comma)
            .unwrap()
            .write_all(b"content")
            .unwrap();

        let groups = vec![DuplicateGroup::new([0u8; 32], 7, vec![file_with_comma])];

        let output = CsvOutput::new(&groups);
        let csv_str = output.to_string().unwrap();

        // Path should be quoted
        assert!(csv_str.contains("\""));
        assert!(csv_str.contains("file,with,comma.txt"));
    }
}
