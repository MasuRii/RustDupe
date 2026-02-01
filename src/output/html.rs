//! HTML output formatter for duplicate scan results.
//!
//! This module provides a high-performance, self-contained HTML report generator
//! using the `askama` template engine.
//!
//! # Features
//!
//! * **Self-contained**: All CSS is embedded in the HTML file for easy sharing.
//! * **Responsive**: The layout adjusts to different screen sizes.
//! * **Interactive**: Includes collapsible sections for each duplicate group.
//! * **Safe**: Automatically escapes file paths to prevent XSS.
//! * **Themed**: Supports dark mode via system media queries.
//!
//! # Usage
//!
//! ```rust,ignore
//! use rustdupe::output::html::HtmlOutput;
//!
//! let output = HtmlOutput::new(&groups, &summary);
//! let html = output.to_html().unwrap();
//! ```

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

    #[test]
    fn test_html_escaping() {
        let now = SystemTime::now();
        // Path with characters that need escaping: <, >, &, ", '
        let tricky_path = PathBuf::from("/test/<script>alert('xss')</script> & \"quote\".txt");
        let groups = vec![DuplicateGroup::new(
            [0u8; 32],
            1024,
            vec![
                FileEntry::new(tricky_path, 1024, now),
                FileEntry::new(PathBuf::from("/test/file2.txt"), 1024, now),
            ],
            Vec::new(),
        )];
        let summary = ScanSummary::default();

        let output = HtmlOutput::new(&groups, &summary);
        let html = output.to_html().expect("Failed to render HTML");

        // Check that the tricky characters are escaped
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("alert(&#x27;xss&#x27;)"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;quote&quot;"));
    }

    #[test]
    fn test_empty_report() {
        let groups = Vec::new();
        let summary = ScanSummary::default();

        let output = HtmlOutput::new(&groups, &summary);
        let html = output.to_html().expect("Failed to render HTML");

        assert!(html.contains("Duplicate Report"));
        assert!(html.contains("0 files"));
        assert!(html.contains("0 B"));
        // Check for absence of group cards in the body
        assert!(!html.contains("class=\"group-card\""));
    }

    #[test]
    fn test_summary_stats_rendering() {
        let summary = ScanSummary {
            total_files: 1234,
            total_size: 1024 * 1024 * 10, // 10 MiB
            duplicate_groups: 50,
            duplicate_files: 100,
            reclaimable_space: 1024 * 1024 * 5, // 5 MiB
            scan_duration: Duration::from_secs(42),
            ..Default::default()
        };

        let output = HtmlOutput::new(&[], &summary);
        let html = output.to_html().expect("Failed to render HTML");

        assert!(html.contains("1234 files"));
        assert!(html.contains("10.5") || html.contains("10.0")); // 10 MiB vs 10 MB
        assert!(html.contains("50")); // duplicate groups
        assert!(html.contains("5.2") || html.contains("5.0")); // reclaimable
    }

    #[test]
    fn test_reference_badge_rendering() {
        let now = SystemTime::now();
        let ref_path = PathBuf::from("/ref/original.jpg");
        let groups = vec![DuplicateGroup::new(
            [0u8; 32],
            1024,
            vec![
                FileEntry::new(ref_path.clone(), 1024, now),
                FileEntry::new(PathBuf::from("/tmp/dupe.jpg"), 1024, now),
            ],
            vec![ref_path],
        )];
        let summary = ScanSummary::default();

        let output = HtmlOutput::new(&groups, &summary);
        let html = output.to_html().expect("Failed to render HTML");

        assert!(html.contains("badge-ref"));
        assert!(html.contains("Reference"));
    }
}
