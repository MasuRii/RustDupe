//! HTML output formatter for duplicate scan results.
//!
//! Provides a self-contained HTML report with embedded CSS, responsive layout,
//! and collapsible sections for reviewing duplicates in a browser.

use std::io::Write;
use std::time::SystemTime;

use askama::Template;
use bytesize::ByteSize;
use chrono::{DateTime, Local};

use crate::duplicates::{DuplicateGroup, ScanSummary};

/// Complete HTML output structure for the Askama template.
#[derive(Template)]
#[template(path = "report.html")]
pub struct HtmlOutput {
    /// Formatted generation timestamp
    pub timestamp: String,
    /// Scan summary statistics
    pub summary: ScanSummary,
    /// Human-readable total size
    pub total_size: String,
    /// Human-readable reclaimable space
    pub reclaimable_space: String,
    /// List of duplicate groups formatted for HTML
    pub groups: Vec<HtmlDuplicateGroup>,
}

/// A duplicate group formatted for HTML presentation.
pub struct HtmlDuplicateGroup {
    /// BLAKE3 hash as hexadecimal string
    pub hash_hex: String,
    /// Human-readable file size (shared by all files)
    pub size_formatted: String,
    /// Detailed file entries for this group
    pub files: Vec<HtmlFileEntry>,
}

/// A file entry formatted for HTML presentation.
pub struct HtmlFileEntry {
    /// Absolute path display string
    pub path_display: String,
    /// Formatted modification time
    pub modified_formatted: String,
    /// Whether this file is in a protected reference directory
    pub is_reference: bool,
}

impl HtmlOutput {
    /// Create a new HTML output from duplicate groups and summary.
    ///
    /// # Arguments
    ///
    /// * `groups` - The duplicate groups found during scanning
    /// * `summary` - The scan summary statistics
    #[must_use]
    pub fn new(groups: &[DuplicateGroup], summary: &ScanSummary) -> Self {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let html_groups = groups
            .iter()
            .map(|g| HtmlDuplicateGroup {
                hash_hex: g.hash_hex(),
                size_formatted: ByteSize::b(g.size).to_string(),
                files: g
                    .files
                    .iter()
                    .map(|f| HtmlFileEntry {
                        path_display: f.path.to_string_lossy().into_owned(),
                        modified_formatted: format_time(f.modified),
                        is_reference: g.is_in_reference_dir(&f.path),
                    })
                    .collect(),
            })
            .collect();

        Self {
            timestamp,
            summary: summary.clone(),
            total_size: ByteSize::b(summary.total_size).to_string(),
            reclaimable_space: ByteSize::b(summary.reclaimable_space).to_string(),
            groups: html_groups,
        }
    }

    /// Generate the HTML string using the embedded template.
    ///
    /// # Errors
    ///
    /// Returns an error if template rendering fails.
    pub fn to_html(&self) -> Result<String, askama::Error> {
        self.render()
    }

    /// Write HTML report to a writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - The writer to output to (e.g., file or stdout)
    ///
    /// # Errors
    ///
    /// Returns an error if rendering or writing fails.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), HtmlOutputError> {
        let html = self.to_html()?;
        writer.write_all(html.as_bytes())?;
        Ok(())
    }
}

/// Format a SystemTime as a local date string.
fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Errors that can occur during HTML output generation.
#[derive(thiserror::Error, Debug)]
pub enum HtmlOutputError {
    /// Template rendering error
    #[error("HTML template error: {0}")]
    Template(#[from] askama::Error),

    /// I/O error during writing
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::FileEntry;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_html_output_new() {
        let now = SystemTime::now();
        let groups = vec![DuplicateGroup::new(
            [0u8; 32],
            1024,
            vec![
                FileEntry::new(PathBuf::from("/test/file1.txt"), 1024, now),
                FileEntry::new(PathBuf::from("/test/file2.txt"), 1024, now),
            ],
            vec![PathBuf::from("/test/file1.txt")],
        )];
        let summary = ScanSummary {
            total_files: 2,
            total_size: 2048,
            duplicate_groups: 1,
            duplicate_files: 1,
            reclaimable_space: 1024,
            scan_duration: Duration::from_secs(1),
            ..Default::default()
        };

        let output = HtmlOutput::new(&groups, &summary);
        assert_eq!(output.summary.total_files, 2);
        assert_eq!(output.groups.len(), 1);
        assert_eq!(output.groups[0].files.len(), 2);
        assert!(output.groups[0].files[0].is_reference);
        assert!(!output.groups[0].files[1].is_reference);
    }

    #[test]
    fn test_to_html() {
        let now = SystemTime::now();
        let groups = vec![DuplicateGroup::new(
            [0xAB; 32],
            1024,
            vec![
                FileEntry::new(PathBuf::from("/test/file1.txt"), 1024, now),
                FileEntry::new(PathBuf::from("/test/file2.txt"), 1024, now),
            ],
            Vec::new(),
        )];
        let summary = ScanSummary {
            total_files: 2,
            total_size: 2048,
            duplicate_groups: 1,
            duplicate_files: 1,
            reclaimable_space: 1024,
            scan_duration: Duration::from_secs(1),
            ..Default::default()
        };

        let output = HtmlOutput::new(&groups, &summary);
        let html = output.to_html().expect("Failed to render HTML");

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Duplicate Report"));
        // bytesize formatting - 2048 bytes and 1024 bytes
        assert!(html.contains("2.0") && (html.contains("KiB") || html.contains("KB")));
        assert!(html.contains("1.0") && (html.contains("KiB") || html.contains("KB")));
        assert!(html.contains("abababab")); // hash
        assert!(html.contains("file1.txt"));
    }
}
