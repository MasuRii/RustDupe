//! Shell script generation for duplicate file deletion.
//!
//! This module enables users to export their deletion selections as a shell script,
//! allowing for manual review and execution of file removals.
//!
//! # Features
//!
//! * **Multi-platform**: Supports POSIX shell scripts (Unix) and PowerShell (Windows).
//! * **Safety-first**: Scripts default to dry-run mode and require a `--confirm` flag.
//! * **Robust Escaping**: Handles spaces, quotes, and special characters in file paths.
//! * **Informative**: Includes comments with file hashes, sizes, and group info.
//! * **Summary**: Displays total deleted count and reclaimed space upon completion.
//!
//! # Usage
//!
//! ```rust,ignore
//! use rustdupe::output::script::{ScriptOutput, ScriptType};
//!
//! let output = ScriptOutput::new(&groups, &summary, ScriptType::detect());
//! output.write_to(&mut std::io::stdout()).unwrap();
//! ```

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::duplicates::{DuplicateGroup, ScanSummary};

/// Type of script to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    /// POSIX-compliant shell script (sh/bash/zsh)
    Posix,
    /// Windows PowerShell script
    PowerShell,
}

impl ScriptType {
    /// Detect the appropriate script type for the current platform.
    #[must_use]
    pub fn detect() -> Self {
        if cfg!(windows) {
            Self::PowerShell
        } else {
            Self::Posix
        }
    }
}

/// Formatter for shell script output.
pub struct ScriptOutput<'a> {
    /// Duplicate groups to include in the script
    pub groups: &'a [DuplicateGroup],
    /// Scan summary for statistics and comments
    pub summary: &'a ScanSummary,
    /// The type of script to generate
    pub script_type: ScriptType,
    /// Optional user selections from a session
    pub user_selections: Option<&'a BTreeSet<PathBuf>>,
}

impl<'a> ScriptOutput<'a> {
    /// Create a new script output formatter.
    #[must_use]
    pub fn new(
        groups: &'a [DuplicateGroup],
        summary: &'a ScanSummary,
        script_type: ScriptType,
    ) -> Self {
        Self {
            groups,
            summary,
            script_type,
            user_selections: None,
        }
    }

    /// Set user selections for the script.
    #[must_use]
    pub fn with_user_selections(mut self, selections: &'a BTreeSet<PathBuf>) -> Self {
        self.user_selections = Some(selections);
        self
    }

    /// Write the generated script to a writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self.script_type {
            ScriptType::Posix => self.write_posix(writer),
            ScriptType::PowerShell => self.write_powershell(writer),
        }
    }

    fn write_posix<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "#!/bin/sh")?;
        writeln!(writer, "# RustDupe Duplicate Deletion Script")?;
        writeln!(
            writer,
            "# Generated on: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )?;
        writeln!(writer, "#")?;
        writeln!(
            writer,
            "# WARNING: This script will PERMANENTLY DELETE files."
        )?;
        writeln!(writer, "# Please review carefully before executing.")?;
        writeln!(writer, "#")?;
        writeln!(
            writer,
            "# Total duplicates found: {}",
            self.summary.duplicate_files
        )?;
        writeln!(
            writer,
            "# Reclaimable space: {}",
            bytesize::ByteSize::b(self.summary.reclaimable_space)
        )?;
        writeln!(writer)?;

        writeln!(writer, "# Default to dry-run mode for safety")?;
        writeln!(writer, "DRY_RUN=1")?;
        writeln!(writer, "if [ \"$1\" = \"--confirm\" ]; then")?;
        writeln!(writer, "    DRY_RUN=0")?;
        writeln!(writer, "fi")?;
        writeln!(writer)?;

        writeln!(writer, "if [ \"$DRY_RUN\" -eq 1 ]; then")?;
        writeln!(
            writer,
            "    echo \"DRY RUN MODE. No files will be deleted.\""
        )?;
        writeln!(
            writer,
            "    echo \"Run with --confirm to actually delete files.\""
        )?;
        writeln!(writer, "    echo")?;
        writeln!(writer, "fi")?;
        writeln!(writer)?;

        writeln!(writer, "DELETED_COUNT=0")?;
        writeln!(writer, "RECLAIMED_BYTES=0")?;
        writeln!(writer)?;

        for (i, group) in self.groups.iter().enumerate() {
            writeln!(
                writer,
                "# Group {}: Hash {}, Size {}",
                i + 1,
                group.hash_hex(),
                bytesize::ByteSize::b(group.size)
            )?;

            let mut group_has_deletion = false;
            for (j, file) in group.files.iter().enumerate() {
                let path_str = escape_posix(&file.path);
                let should_delete = if let Some(selections) = self.user_selections {
                    selections.contains(&file.path)
                } else {
                    // Default logic: keep reference files and the first file if no reference files exist
                    let has_ref_in_group = group
                        .files
                        .iter()
                        .any(|f| group.is_in_reference_dir(&f.path));
                    if has_ref_in_group {
                        // Keep ALL reference files, delete others
                        !group.is_in_reference_dir(&file.path)
                    } else {
                        // No reference files, keep first, delete others
                        j > 0
                    }
                };

                if should_delete {
                    writeln!(writer, "# DELETE: {}", path_str)?;
                    writeln!(writer, "if [ \"$DRY_RUN\" -eq 0 ]; then")?;
                    writeln!(writer, "    rm {} && \\", path_str)?;
                    writeln!(writer, "    DELETED_COUNT=$((DELETED_COUNT + 1)) && \\")?;
                    writeln!(
                        writer,
                        "    RECLAIMED_BYTES=$((RECLAIMED_BYTES + {}))",
                        group.size
                    )?;
                    writeln!(writer, "else")?;
                    writeln!(writer, "    echo \"would delete: {}\"", path_str)?;
                    writeln!(writer, "fi")?;
                    group_has_deletion = true;
                } else {
                    writeln!(writer, "# KEEP:   {}", path_str)?;
                }
            }
            if group_has_deletion {
                writeln!(writer)?;
            }
        }

        writeln!(writer, "if [ \"$DRY_RUN\" -eq 0 ]; then")?;
        writeln!(
            writer,
            "    echo \"Deletion complete. Deleted $DELETED_COUNT files.\""
        )?;
        writeln!(writer, "    echo \"Reclaimed $RECLAIMED_BYTES bytes.\"")?;
        writeln!(writer, "else")?;
        writeln!(
            writer,
            "    echo \"Dry run complete. No files were deleted.\""
        )?;
        writeln!(writer, "fi")?;

        Ok(())
    }

    fn write_powershell<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "# RustDupe Duplicate Deletion Script")?;
        writeln!(
            writer,
            "# Generated on: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )?;
        writeln!(writer, "#")?;
        writeln!(
            writer,
            "# WARNING: This script will PERMANENTLY DELETE files."
        )?;
        writeln!(writer, "# Please review carefully before executing.")?;
        writeln!(writer, "#")?;
        writeln!(
            writer,
            "# Total duplicates found: {}",
            self.summary.duplicate_files
        )?;
        writeln!(
            writer,
            "# Reclaimable space: {}",
            bytesize::ByteSize::b(self.summary.reclaimable_space)
        )?;
        writeln!(writer)?;

        writeln!(writer, "# Default to dry-run mode for safety")?;
        writeln!(writer, "$DryRun = $true")?;
        writeln!(writer, "if ($args[0] -eq \"--confirm\") {{")?;
        writeln!(writer, "    $DryRun = $false")?;
        writeln!(writer, "}}")?;
        writeln!(writer)?;

        writeln!(writer, "if ($DryRun) {{")?;
        writeln!(
            writer,
            "    Write-Host \"DRY RUN MODE. No files will be deleted.\""
        )?;
        writeln!(
            writer,
            "    Write-Host \"Run with --confirm to actually delete files.\""
        )?;
        writeln!(writer, "    Write-Host \"\"")?;
        writeln!(writer, "}}")?;
        writeln!(writer)?;

        writeln!(writer, "$DeletedCount = 0")?;
        writeln!(writer, "$ReclaimedBytes = 0")?;
        writeln!(writer)?;

        for (i, group) in self.groups.iter().enumerate() {
            writeln!(
                writer,
                "# Group {}: Hash {}, Size {}",
                i + 1,
                group.hash_hex(),
                bytesize::ByteSize::b(group.size)
            )?;

            let mut group_has_deletion = false;
            for (j, file) in group.files.iter().enumerate() {
                let path_str = escape_powershell(&file.path);
                let should_delete = if let Some(selections) = self.user_selections {
                    selections.contains(&file.path)
                } else {
                    // Default logic: keep reference files and the first file if no reference files exist
                    let has_ref_in_group = group
                        .files
                        .iter()
                        .any(|f| group.is_in_reference_dir(&f.path));
                    if has_ref_in_group {
                        // Keep ALL reference files, delete others
                        !group.is_in_reference_dir(&file.path)
                    } else {
                        // No reference files, keep first, delete others
                        j > 0
                    }
                };

                if should_delete {
                    writeln!(writer, "# DELETE: {}", path_str)?;
                    writeln!(writer, "if (-not $DryRun) {{")?;
                    writeln!(
                        writer,
                        "    Remove-Item -Path {} -ErrorAction SilentlyContinue",
                        path_str
                    )?;
                    writeln!(writer, "    if ($?) {{")?;
                    writeln!(writer, "        $DeletedCount++")?;
                    writeln!(writer, "        $ReclaimedBytes += {}", group.size)?;
                    writeln!(writer, "    }}")?;
                    writeln!(writer, "}} else {{")?;
                    writeln!(writer, "    Write-Host \"would delete: {}\"", path_str)?;
                    writeln!(writer, "}}")?;
                    group_has_deletion = true;
                } else {
                    writeln!(writer, "# KEEP:   {}", path_str)?;
                }
            }
            if group_has_deletion {
                writeln!(writer)?;
            }
        }

        writeln!(writer, "if (-not $DryRun) {{")?;
        writeln!(
            writer,
            "    Write-Host \"Deletion complete. Deleted $DeletedCount files.\""
        )?;
        writeln!(
            writer,
            "    Write-Host \"Reclaimed $ReclaimedBytes bytes.\""
        )?;
        writeln!(writer, "}} else {{")?;
        writeln!(
            writer,
            "    Write-Host \"Dry run complete. No files were deleted.\""
        )?;
        writeln!(writer, "fi")?;

        Ok(())
    }
}

fn escape_posix(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Wrap in single quotes, escape single quotes as '\''
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn escape_powershell(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Wrap in single quotes, escape single quotes as ''
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::FileEntry;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn setup_test_data() -> (Vec<DuplicateGroup>, ScanSummary) {
        let now = SystemTime::now();
        let groups = vec![DuplicateGroup::new(
            [0u8; 32],
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
        (groups, summary)
    }

    #[test]
    fn test_escape_posix() {
        assert_eq!(escape_posix(Path::new("/foo/bar.txt")), "'/foo/bar.txt'");
        assert_eq!(
            escape_posix(Path::new("/foo's/bar.txt")),
            "'/foo'\\''s/bar.txt'"
        );
        assert_eq!(
            escape_posix(Path::new("/foo bar/baz.txt")),
            "'/foo bar/baz.txt'"
        );
        assert_eq!(
            escape_posix(Path::new("/foo$bar/`baz`.txt")),
            "'/foo$bar/`baz`.txt'"
        );
    }

    #[test]
    fn test_escape_powershell() {
        assert_eq!(
            escape_powershell(Path::new("C:\\foo\\bar.txt")),
            "'C:\\foo\\bar.txt'"
        );
        assert_eq!(
            escape_powershell(Path::new("C:\\foo's\\bar.txt")),
            "'C:\\foo''s\\bar.txt'"
        );
        assert_eq!(
            escape_powershell(Path::new("C:\\foo bar\\baz.txt")),
            "'C:\\foo bar\\baz.txt'"
        );
        assert_eq!(
            escape_powershell(Path::new("C:\\foo$bar\\`baz`.txt")),
            "'C:\\foo$bar\\`baz`.txt'"
        );
    }

    #[test]
    fn test_complex_script_generation() {
        let now = SystemTime::now();
        let groups = vec![
            DuplicateGroup::new(
                [1u8; 32],
                100,
                vec![
                    FileEntry::new(PathBuf::from("/keep/me.txt"), 100, now),
                    FileEntry::new(PathBuf::from("/delete/me.txt"), 100, now),
                    FileEntry::new(PathBuf::from("/delete/too.txt"), 100, now),
                ],
                Vec::new(),
            ),
            DuplicateGroup::new(
                [2u8; 32],
                200,
                vec![
                    FileEntry::new(PathBuf::from("/path with spaces/file1.txt"), 200, now),
                    FileEntry::new(PathBuf::from("/path with spaces/file2.txt"), 200, now),
                ],
                Vec::new(),
            ),
        ];

        let summary = ScanSummary {
            duplicate_files: 3,
            reclaimable_space: 400, // (100*2) + 200
            ..Default::default()
        };

        // Test POSIX with default selections
        let output = ScriptOutput::new(&groups, &summary, ScriptType::Posix);
        let mut buffer = Vec::new();
        output.write_to(&mut buffer).unwrap();
        let script = String::from_utf8(buffer).unwrap();

        assert!(script.contains("# KEEP:   '/keep/me.txt'"));
        assert!(script.contains("# DELETE: '/delete/me.txt'"));
        assert!(script.contains("# DELETE: '/delete/too.txt'"));
        assert!(script.contains("# KEEP:   '/path with spaces/file1.txt'"));
        assert!(script.contains("# DELETE: '/path with spaces/file2.txt'"));
        assert!(script.contains("rm '/delete/me.txt'"));
        assert!(script.contains("rm '/delete/too.txt'"));
        assert!(script.contains("rm '/path with spaces/file2.txt'"));

        // Test PowerShell with user selections
        let mut selections = BTreeSet::new();
        selections.insert(PathBuf::from("/keep/me.txt"));
        selections.insert(PathBuf::from("/path with spaces/file1.txt"));

        let output = ScriptOutput::new(&groups, &summary, ScriptType::PowerShell)
            .with_user_selections(&selections);
        let mut buffer = Vec::new();
        output.write_to(&mut buffer).unwrap();
        let script = String::from_utf8(buffer).unwrap();

        assert!(script.contains("# DELETE: '/keep/me.txt'"));
        assert!(script.contains("# KEEP:   '/delete/me.txt'"));
        assert!(script.contains("# KEEP:   '/delete/too.txt'"));
        assert!(script.contains("# DELETE: '/path with spaces/file1.txt'"));
        assert!(script.contains("# KEEP:   '/path with spaces/file2.txt'"));
        assert!(script.contains("Remove-Item -Path '/keep/me.txt'"));
        assert!(script.contains("Remove-Item -Path '/path with spaces/file1.txt'"));
    }

    #[test]
    fn test_posix_generation() {
        let (groups, summary) = setup_test_data();
        let output = ScriptOutput::new(&groups, &summary, ScriptType::Posix);
        let mut buffer = Vec::new();
        output.write_to(&mut buffer).unwrap();
        let script = String::from_utf8(buffer).unwrap();

        assert!(script.starts_with("#!/bin/sh"));
        assert!(script.contains("DRY_RUN=1"));
        assert!(script.contains("would delete: '/test/file2.txt'"));
        assert!(script.contains("rm '/test/file2.txt'"));
        assert!(script.contains("# KEEP:   '/test/file1.txt'"));
        assert!(script.contains("--confirm"));
    }

    #[test]
    fn test_powershell_generation() {
        let (groups, summary) = setup_test_data();
        let output = ScriptOutput::new(&groups, &summary, ScriptType::PowerShell);
        let mut buffer = Vec::new();
        output.write_to(&mut buffer).unwrap();
        let script = String::from_utf8(buffer).unwrap();

        assert!(script.contains("$DryRun = $true"));
        assert!(script.contains("would delete: '/test/file2.txt'"));
        assert!(script.contains("Remove-Item -Path '/test/file2.txt'"));
        assert!(script.contains("# KEEP:   '/test/file1.txt'"));
        assert!(script.contains("--confirm"));
    }

    #[test]
    fn test_with_user_selections() {
        let (groups, summary) = setup_test_data();
        let mut selections = BTreeSet::new();
        selections.insert(PathBuf::from("/test/file1.txt")); // Select first one instead of second

        let output = ScriptOutput::new(&groups, &summary, ScriptType::Posix)
            .with_user_selections(&selections);
        let mut buffer = Vec::new();
        output.write_to(&mut buffer).unwrap();
        let script = String::from_utf8(buffer).unwrap();

        assert!(script.contains("# DELETE: '/test/file1.txt'"));
        assert!(script.contains("# KEEP:   '/test/file2.txt'"));
    }

    #[test]
    fn test_reference_directory_selection() {
        let now = SystemTime::now();
        let ref_path = PathBuf::from("/ref/original.txt");
        let groups = vec![DuplicateGroup::new(
            [1u8; 32],
            100,
            vec![
                FileEntry::new(ref_path.clone(), 100, now),
                FileEntry::new(PathBuf::from("/tmp/dupe1.txt"), 100, now),
                FileEntry::new(PathBuf::from("/tmp/dupe2.txt"), 100, now),
            ],
            vec![ref_path.clone()],
        )];

        let summary = ScanSummary {
            duplicate_files: 2,
            reclaimable_space: 200,
            ..Default::default()
        };

        let output = ScriptOutput::new(&groups, &summary, ScriptType::Posix);
        let mut buffer = Vec::new();
        output.write_to(&mut buffer).unwrap();
        let script = String::from_utf8(buffer).unwrap();

        // Should keep the reference file even if it's the first one
        assert!(script.contains("# KEEP:   '/ref/original.txt'"));
        assert!(script.contains("# DELETE: '/tmp/dupe1.txt'"));
        assert!(script.contains("# DELETE: '/tmp/dupe2.txt'"));

        // Now test with reference file as NOT the first one
        let groups2 = vec![DuplicateGroup::new(
            [1u8; 32],
            100,
            vec![
                FileEntry::new(PathBuf::from("/tmp/dupe1.txt"), 100, now),
                FileEntry::new(ref_path.clone(), 100, now),
                FileEntry::new(PathBuf::from("/tmp/dupe2.txt"), 100, now),
            ],
            vec![ref_path],
        )];

        let output2 = ScriptOutput::new(&groups2, &summary, ScriptType::Posix);
        let mut buffer2 = Vec::new();
        output2.write_to(&mut buffer2).unwrap();
        let script2 = String::from_utf8(buffer2).unwrap();

        // Should keep the reference file and DELETE the first file (since there's a reference file)
        assert!(script2.contains("# DELETE: '/tmp/dupe1.txt'"));
        assert!(script2.contains("# KEEP:   '/ref/original.txt'"));
        assert!(script2.contains("# DELETE: '/tmp/dupe2.txt'"));
    }
}
