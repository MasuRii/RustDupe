//! Command-line interface definitions for RustDupe.
//!
//! This module defines all CLI arguments, subcommands, and options using the clap derive API.
//! The CLI follows standard conventions with global options (verbosity, color) and
//! subcommands for different operations.
//!
//! # Example
//!
//! ```bash
//! # Scan a directory with TUI output (default)
//! rustdupe scan ~/Downloads
//!
//! # Scan with JSON output for scripting
//! rustdupe scan ~/Downloads --output json
//!
//! # Scan with size filters
//! rustdupe scan ~/Downloads --min-size 1MB --max-size 1GB
//!
//! # Verbose mode for debugging
//! rustdupe -v scan ~/Downloads
//!
//! # Use vim keybinding profile
//! rustdupe --keybinding-profile vim scan ~/Downloads
//! ```

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::tui::keybindings::KeybindingProfile;

/// A named directory group mapping a name to a path.
///
/// Used with the `--group` flag: `--group photos=/path/to/photos`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryGroup {
    /// The group name (e.g., "photos")
    pub name: String,
    /// The path to scan
    pub path: PathBuf,
}

impl DirectoryGroup {
    /// Create a new directory group.
    #[must_use]
    pub fn new(name: String, path: PathBuf) -> Self {
        Self { name, path }
    }
}

/// Parse a group specification in the format `NAME=PATH`.
///
/// # Examples
///
/// ```
/// use rustdupe::cli::parse_group;
///
/// let group = parse_group("photos=/home/user/photos").unwrap();
/// assert_eq!(group.name, "photos");
/// assert_eq!(group.path.to_string_lossy(), "/home/user/photos");
///
/// // Invalid format (no =)
/// assert!(parse_group("invalid").is_err());
///
/// // Empty name
/// assert!(parse_group("=/path").is_err());
///
/// // Empty path
/// assert!(parse_group("name=").is_err());
/// ```
pub fn parse_group(s: &str) -> Result<DirectoryGroup, String> {
    let s = s.trim();
    let eq_pos = s.find('=').ok_or_else(|| {
        format!("Invalid group format: '{s}'. Expected NAME=PATH (e.g., photos=/path/to/photos)")
    })?;

    let name = s[..eq_pos].trim();
    let path = s[eq_pos + 1..].trim();

    if name.is_empty() {
        return Err("Group name cannot be empty".to_string());
    }

    if path.is_empty() {
        return Err("Group path cannot be empty".to_string());
    }

    // Validate name contains only alphanumeric characters and underscores
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "Invalid group name '{name}': only alphanumeric characters, underscores, and hyphens are allowed"
        ));
    }

    Ok(DirectoryGroup {
        name: name.to_string(),
        path: PathBuf::from(path),
    })
}

/// Build a mapping from canonical paths to group names.
///
/// Returns a HashMap where keys are canonical directory paths and values are group names.
/// This is used during scanning to assign group names to discovered files.
///
/// # Errors
///
/// Returns an error if any group path fails to canonicalize (doesn't exist, permission denied, etc.)
pub fn build_group_map(groups: &[DirectoryGroup]) -> Result<HashMap<PathBuf, String>, String> {
    let mut map = HashMap::new();
    for group in groups {
        let canonical = group.path.canonicalize().map_err(|e| {
            format!(
                "Failed to resolve group path '{}' for group '{}': {}",
                group.path.display(),
                group.name,
                e
            )
        })?;
        map.insert(canonical, group.name.clone());
    }
    Ok(map)
}

/// Smart duplicate file finder with interactive TUI.
///
/// RustDupe finds duplicate files using content hashing (BLAKE3), provides an
/// interactive TUI for review, and supports safe deletion via system trash.
#[derive(Debug, Parser)]
#[command(name = "rustdupe")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Increase verbosity level (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long = "no-color", global = true, env = "NO_COLOR", value_parser = clap::builder::BoolishValueParser::new())]
    pub no_color: bool,

    /// Enable colored output
    #[arg(long = "color", overrides_with = "no_color", hide = true)]
    pub color: bool,

    /// TUI theme (light, dark, auto)
    #[arg(long = "theme", value_enum, global = true)]
    pub theme: Option<ThemeArg>,

    /// Keybinding profile for TUI navigation (universal, vim, standard, emacs)
    ///
    /// - universal: Both vim-style (hjkl) AND arrow keys (default)
    /// - vim: Vim-style navigation only (hjkl)
    /// - standard: Arrow keys and standard shortcuts only
    /// - emacs: Emacs-style keybindings (Ctrl-n/p)
    #[arg(
        long = "keybinding-profile",
        alias = "keys",
        value_enum,
        global = true,
        env = "RUSTDUPE_KEYBINDING_PROFILE"
    )]
    pub keybinding_profile: Option<KeybindingProfile>,

    /// Load a named configuration profile from the config file
    ///
    /// Profiles are defined in the config file under [profile.NAME] sections.
    /// Profile settings override base configuration but are overridden by CLI flags.
    #[arg(long, value_name = "NAME", global = true)]
    pub profile: Option<String>,

    /// List all available configuration profiles and exit
    #[arg(long, global = true)]
    pub list_profiles: bool,

    /// Enable accessible mode for screen reader compatibility
    ///
    /// When enabled:
    /// - Uses simple ASCII borders instead of Unicode box-drawing characters
    /// - Disables animations and spinners
    /// - Simplifies progress output (no cursor movement)
    /// - Reduces screen refresh rate for better screen reader performance
    #[arg(long = "accessible", global = true)]
    pub accessible: bool,

    /// Disable accessible mode
    #[arg(long = "no-accessible", overrides_with = "accessible", hide = true)]
    pub no_accessible: bool,

    /// Output errors as JSON instead of plain text
    #[arg(long = "json-errors", global = true)]
    pub json_errors: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands for RustDupe.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Scan a directory for duplicate files
    Scan(Box<ScanArgs>),
    /// Load a previously saved session
    Load(LoadArgs),
}

/// Arguments for the scan subcommand.
#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Directory paths to scan for duplicates
    ///
    /// Multiple directories can be specified and will be scanned together.
    /// Duplicates will be found across all specified directories.
    ///
    /// Example: rustdupe scan /path/1 /path/2 /path/3
    #[arg(value_name = "PATH", num_args = 1.., required_unless_present = "load_session")]
    pub paths: Vec<PathBuf>,

    /// Load a previously saved session instead of scanning
    #[arg(
        long,
        value_name = "SESSION_FILE",
        conflicts_with = "paths",
        help_heading = "Scanning Options"
    )]
    pub load_session: Option<PathBuf>,

    /// Save scan results to a session file
    #[arg(long, value_name = "PATH", help_heading = "Output Options")]
    pub save_session: Option<PathBuf>,

    /// Output format (tui for interactive, json/csv for scripting, session for persistence, html for report, script for deletion)
    #[arg(short, long, value_enum, help_heading = "Output Options")]
    pub output: Option<OutputFormat>,

    /// Write output to a file instead of stdout
    #[arg(long, value_name = "PATH", help_heading = "Output Options")]
    pub output_file: Option<PathBuf>,

    /// Type of deletion script to generate
    #[arg(long, value_enum, value_name = "TYPE", help_heading = "Output Options")]
    pub script_type: Option<ScriptTypeArg>,

    /// Minimum file size to consider (e.g., 1KB, 1MB, 1GB)
    ///
    /// Supports suffixes: B, KB, KiB, MB, MiB, GB, GiB, TB, TiB
    #[arg(long, value_name = "SIZE", value_parser = parse_size, help_heading = "Filtering Options")]
    pub min_size: Option<u64>,

    /// Maximum file size to consider (e.g., 1KB, 1MB, 1GB)
    ///
    /// Supports suffixes: B, KB, KiB, MB, MiB, GB, GiB, TB, TiB
    #[arg(long, value_name = "SIZE", value_parser = parse_size, help_heading = "Filtering Options")]
    pub max_size: Option<u64>,

    /// Only include files modified after this date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = parse_date, help_heading = "Filtering Options")]
    pub newer_than: Option<std::time::SystemTime>,

    /// Only include files modified before this date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = parse_date, help_heading = "Filtering Options")]
    pub older_than: Option<std::time::SystemTime>,

    /// Regex patterns to include (filename must match at least one)
    ///
    /// Example: --regex ".*\.jpg$"
    #[arg(
        long = "regex",
        alias = "regex-include",
        value_name = "PATTERN",
        help_heading = "Filtering Options"
    )]
    pub regex_include: Vec<String>,

    /// Regex patterns to exclude (filename must not match any)
    ///
    /// Example: --regex-exclude "temp_.*"
    #[arg(
        long = "regex-exclude",
        value_name = "PATTERN",
        help_heading = "Filtering Options"
    )]
    pub regex_exclude: Vec<String>,

    /// Filter by file type categories (can be specified multiple times)
    #[arg(
        long = "file-type",
        value_enum,
        value_name = "TYPE",
        help_heading = "Filtering Options"
    )]
    pub file_types: Vec<FileType>,

    /// Glob patterns to ignore (can be specified multiple times)
    ///
    /// These patterns are added to any .gitignore patterns found.
    #[arg(
        short,
        long = "ignore",
        value_name = "PATTERN",
        help_heading = "Filtering Options"
    )]
    pub ignore_patterns: Vec<String>,

    /// Follow symbolic links during scan
    ///
    /// Warning: May cause infinite loops if symlinks form cycles.
    #[arg(long = "follow-symlinks", help_heading = "Scanning Options")]
    pub follow_symlinks: bool,

    /// Do not follow symbolic links during scan
    #[arg(
        long = "no-follow-symlinks",
        overrides_with = "follow_symlinks",
        hide = true
    )]
    pub no_follow_symlinks: bool,

    /// Skip hidden files and directories (starting with .)
    #[arg(long = "skip-hidden", help_heading = "Scanning Options")]
    pub skip_hidden: bool,

    /// Do not skip hidden files and directories
    #[arg(long = "no-skip-hidden", overrides_with = "skip_hidden", hide = true)]
    pub no_skip_hidden: bool,

    /// Number of I/O threads for hashing (default: 4)
    ///
    /// Lower values reduce disk thrashing on HDDs.
    #[arg(
        long = "io-threads",
        value_name = "N",
        help_heading = "Scanning Options"
    )]
    pub io_threads: Option<usize>,

    /// Enable paranoid mode: byte-by-byte verification after hash match
    ///
    /// Slower but guarantees no hash collisions.
    #[arg(long = "paranoid", help_heading = "Scanning Options")]
    pub paranoid: bool,

    /// Disable paranoid mode
    #[arg(long = "no-paranoid", overrides_with = "paranoid", hide = true)]
    pub no_paranoid: bool,

    /// Use permanent deletion instead of moving to trash
    ///
    /// Warning: Files cannot be recovered after permanent deletion.
    #[arg(long = "permanent", help_heading = "Safety & Deletion Options")]
    pub permanent: bool,

    /// Use system trash instead of permanent deletion
    #[arg(long = "no-permanent", overrides_with = "permanent", hide = true)]
    pub no_permanent: bool,

    /// Skip confirmation prompts (required with --permanent in non-interactive mode)
    #[arg(short = 'y', long = "yes", help_heading = "Safety & Deletion Options")]
    pub yes: bool,

    /// Path to the hash cache database
    ///
    /// If not specified, a default platform-specific path is used.
    #[arg(long = "cache", value_name = "PATH", help_heading = "Cache Options")]
    pub cache: Option<PathBuf>,

    /// Disable hash caching
    #[arg(
        long = "no-cache",
        conflicts_with = "cache",
        help_heading = "Cache Options"
    )]
    pub no_cache: bool,

    /// Enable hash caching
    #[arg(long = "enable-cache", overrides_with = "no_cache", hide = true)]
    pub enable_cache: bool,

    /// Clear the hash cache before scanning
    #[arg(long = "clear-cache", help_heading = "Cache Options")]
    pub clear_cache: bool,

    /// Do not perform any deletions (read-only mode)
    #[arg(
        long = "dry-run",
        alias = "analyze-only",
        help_heading = "Safety & Deletion Options"
    )]
    pub dry_run: bool,

    /// Disable read-only mode (allow deletions)
    #[arg(long = "no-dry-run", overrides_with = "dry_run", hide = true)]
    pub no_dry_run: bool,

    /// Reference directories (files here are never selected for deletion)
    ///
    /// Example: --reference /backups/photos
    ///
    /// Can be specified multiple times. Files in these directories will be
    /// marked as protected and cannot be selected for deletion.
    #[arg(
        long = "reference",
        value_name = "PATH",
        help_heading = "Safety & Deletion Options"
    )]
    pub reference_paths: Vec<PathBuf>,

    /// Named directory groups for organizing and batch-selecting duplicates
    ///
    /// Format: NAME=PATH (e.g., --group photos=/path/to/photos)
    ///
    /// Can be specified multiple times. Group names will be displayed in the TUI
    /// and can be used for batch selection operations.
    ///
    /// Example:
    ///   rustdupe scan --group photos=/Photos --group docs=/Documents
    #[arg(
        long = "group",
        value_name = "NAME=PATH",
        value_parser = parse_group,
        help_heading = "Scanning Options"
    )]
    pub groups: Vec<DirectoryGroup>,

    /// Fail-fast on any error during scan
    #[arg(long = "strict", help_heading = "Scanning Options")]
    pub strict: bool,

    /// Continue scan on errors (default)
    #[arg(long = "no-strict", overrides_with = "strict", hide = true)]
    pub no_strict: bool,
}

/// Arguments for the load subcommand.
#[derive(Debug, Args)]
pub struct LoadArgs {
    /// Session file to load
    #[arg(value_name = "SESSION_FILE")]
    pub path: PathBuf,

    /// Output format (tui for interactive, json/csv for scripting, html for report, script for deletion)
    #[arg(short, long, value_enum, help_heading = "Output Options")]
    pub output: Option<OutputFormat>,

    /// Write output to a file instead of stdout
    #[arg(long, value_name = "PATH", help_heading = "Output Options")]
    pub output_file: Option<PathBuf>,

    /// Type of deletion script to generate
    #[arg(long, value_enum, value_name = "TYPE", help_heading = "Output Options")]
    pub script_type: Option<ScriptTypeArg>,

    /// Do not perform any deletions (read-only mode)
    #[arg(long, alias = "analyze-only", help_heading = "Safety Options")]
    pub dry_run: bool,

    /// Disable read-only mode (allow deletions)
    #[arg(long = "no-dry-run", overrides_with = "dry_run", hide = true)]
    pub no_dry_run: bool,
}

/// Output format for scan results.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Interactive terminal user interface
    #[default]
    Tui,
    /// JSON output for scripting
    Json,
    /// CSV output for spreadsheets
    Csv,
    /// HTML report for browser viewing
    Html,
    /// Session file format for persistence
    Session,
    /// Shell script for deletion
    Script,
}

/// Script type for deletion script generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptTypeArg {
    /// POSIX-compliant shell script (sh/bash/zsh)
    Posix,
    /// Windows PowerShell script
    Powershell,
}

/// File type categories for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    /// Image files (jpg, png, etc.)
    Images,
    /// Video files (mp4, mkv, etc.)
    Videos,
    /// Audio files (mp3, wav, etc.)
    Audio,
    /// Document files (pdf, docx, etc.)
    Documents,
    /// Archive files (zip, tar, etc.)
    Archives,
}

/// TUI theme options.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ThemeArg {
    /// Use terminal's default color scheme or detect automatically
    #[default]
    Auto,
    /// High-contrast light theme
    Light,
    /// High-contrast dark theme
    Dark,
}

impl From<FileType> for crate::scanner::FileCategory {
    fn from(t: FileType) -> Self {
        match t {
            FileType::Images => crate::scanner::FileCategory::Images,
            FileType::Videos => crate::scanner::FileCategory::Videos,
            FileType::Audio => crate::scanner::FileCategory::Audio,
            FileType::Documents => crate::scanner::FileCategory::Documents,
            FileType::Archives => crate::scanner::FileCategory::Archives,
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Tui => write!(f, "tui"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Html => write!(f, "html"),
            OutputFormat::Session => write!(f, "session"),
            OutputFormat::Script => write!(f, "script"),
        }
    }
}

/// Parse a human-readable size string into bytes.
///
/// Supports suffixes: B, KB, KiB, MB, MiB, GB, GiB, TB, TiB
/// Case-insensitive. Numbers without suffix are treated as bytes.
///
/// # Examples
///
/// ```
/// use rustdupe::cli::parse_size;
///
/// assert_eq!(parse_size("1024").unwrap(), 1024);
/// assert_eq!(parse_size("1KB").unwrap(), 1000);
/// assert_eq!(parse_size("1KiB").unwrap(), 1024);
/// assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
/// assert_eq!(parse_size("1MiB").unwrap(), 1_048_576);
/// ```
/// # Errors
///
/// Returns an error if the string is empty, contains an invalid number,
/// a negative number, or an unknown size suffix.
pub fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Size cannot be empty".to_string());
    }

    // Find where the number ends and the suffix begins
    let (num_str, suffix) = match s.find(|c: char| !c.is_ascii_digit() && c != '.') {
        Some(idx) => (&s[..idx], s[idx..].trim().to_uppercase()),
        None => (s, String::new()),
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| format!("Invalid number: '{num_str}'"))?;

    if num < 0.0 {
        return Err("Size cannot be negative".to_string());
    }

    let multiplier: u64 = match suffix.as_str() {
        "" | "B" => 1,
        "KB" | "K" => 1_000,
        "KIB" => 1_024,
        "MB" | "M" => 1_000_000,
        "MIB" => 1_048_576,
        "GB" | "G" => 1_000_000_000,
        "GIB" => 1_073_741_824,
        "TB" | "T" => 1_000_000_000_000,
        "TIB" => 1_099_511_627_776,
        _ => return Err(format!("Unknown size suffix: '{suffix}'")),
    };

    Ok((num * multiplier as f64) as u64)
}

/// Parse a date string in YYYY-MM-DD format into SystemTime.
pub fn parse_date(s: &str) -> Result<std::time::SystemTime, String> {
    use chrono::{NaiveDate, TimeZone, Utc};
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|d| {
            // Use 00:00:00 UTC for the date
            let dt = Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap());
            std::time::SystemTime::from(dt)
        })
        .map_err(|e| format!("Invalid date format (expected YYYY-MM-DD): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_bytes() {
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert_eq!(parse_size("1024B").unwrap(), 1024);
        assert_eq!(parse_size("0").unwrap(), 0);
    }

    #[test]
    fn test_parse_size_kilobytes() {
        assert_eq!(parse_size("1KB").unwrap(), 1_000);
        assert_eq!(parse_size("1K").unwrap(), 1_000);
        assert_eq!(parse_size("1KiB").unwrap(), 1_024);
        assert_eq!(parse_size("1kib").unwrap(), 1_024); // Case insensitive
    }

    #[test]
    fn test_parse_size_megabytes() {
        assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
        assert_eq!(parse_size("1MiB").unwrap(), 1_048_576);
        assert_eq!(parse_size("10MB").unwrap(), 10_000_000);
    }

    #[test]
    fn test_parse_size_gigabytes() {
        assert_eq!(parse_size("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_size("1GiB").unwrap(), 1_073_741_824);
    }

    #[test]
    fn test_parse_size_terabytes() {
        assert_eq!(parse_size("1TB").unwrap(), 1_000_000_000_000);
        assert_eq!(parse_size("1TiB").unwrap(), 1_099_511_627_776);
    }

    #[test]
    fn test_parse_size_fractional() {
        assert_eq!(parse_size("1.5MB").unwrap(), 1_500_000);
        assert_eq!(parse_size("0.5GB").unwrap(), 500_000_000);
    }

    #[test]
    fn test_parse_size_with_whitespace() {
        assert_eq!(parse_size("  1024  ").unwrap(), 1024);
        assert_eq!(parse_size("1 MB").unwrap(), 1_000_000);
    }

    #[test]
    fn test_parse_size_errors() {
        assert!(parse_size("").is_err());
        assert!(parse_size("abc").is_err());
        assert!(parse_size("1XB").is_err());
        assert!(parse_size("-1MB").is_err());
    }

    #[test]
    fn test_cli_parse_help() {
        // Verify that help can be parsed without panicking
        let result = Cli::try_parse_from(["rustdupe", "--help"]);
        // --help causes an early exit, which is an error in try_parse_from
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_parse_scan_basic() {
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/some/path"]).unwrap();
        assert_eq!(cli.verbose, 0);
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.paths, vec![PathBuf::from("/some/path")]);
                assert_eq!(args.output, None); // default is None now
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_with_options() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "-v",
            "scan",
            "/path",
            "--output",
            "json",
            "--min-size",
            "1MB",
            "--max-size",
            "1GB",
            "--newer-than",
            "2026-01-01",
            "--older-than",
            "2026-12-31",
            "--ignore",
            "*.tmp",
            "--ignore",
            "node_modules",
            "--regex",
            "foo.*",
            "--regex-exclude",
            "bar.*",
        ])
        .unwrap();

        assert_eq!(cli.verbose, 1);

        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.output, Some(OutputFormat::Json));
                assert_eq!(args.min_size, Some(1_000_000));
                assert_eq!(args.max_size, Some(1_000_000_000));
                assert!(args.newer_than.is_some());
                assert!(args.older_than.is_some());
                assert_eq!(args.ignore_patterns, vec!["*.tmp", "node_modules"]);
                assert_eq!(args.regex_include, vec!["foo.*"]);
                assert_eq!(args.regex_exclude, vec!["bar.*"]);
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_file_types() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "/path",
            "--file-type",
            "images",
            "--file-type",
            "documents",
        ])
        .unwrap();

        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.file_types, vec![FileType::Images, FileType::Documents]);
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_parse_date() {
        assert!(parse_date("2026-02-01").is_ok());
        assert!(parse_date("2026-02-31").is_err()); // Invalid day
        assert!(parse_date("not-a-date").is_err());
    }

    #[test]
    fn test_cli_parse_scan_script() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "/path",
            "--output",
            "script",
            "--script-type",
            "posix",
        ])
        .unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.output, Some(OutputFormat::Script));
                assert_eq!(args.script_type, Some(ScriptTypeArg::Posix));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_load_script() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "load",
            "session.json",
            "--output",
            "script",
            "--script-type",
            "powershell",
        ])
        .unwrap();
        match cli.command {
            Commands::Load(args) => {
                assert_eq!(args.output, Some(OutputFormat::Script));
                assert_eq!(args.script_type, Some(ScriptTypeArg::Powershell));
            }
            _ => panic!("Expected Load command"),
        }
    }

    #[test]
    fn test_cli_quiet_conflicts_with_verbose() {
        let result = Cli::try_parse_from(["rustdupe", "-v", "-q", "scan", "/path"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_parse_scan_csv() {
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/path", "--output", "csv"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.output, Some(OutputFormat::Csv));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_quiet() {
        let cli = Cli::try_parse_from(["rustdupe", "-q", "scan", "/path"]).unwrap();
        assert!(cli.quiet);
        assert_eq!(cli.verbose, 0);
    }

    #[test]
    fn test_cli_parse_scan_all_flags() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "/path",
            "--follow-symlinks",
            "--skip-hidden",
            "--io-threads",
            "8",
            "--paranoid",
            "--permanent",
            "--yes",
        ])
        .unwrap();

        match cli.command {
            Commands::Scan(args) => {
                assert!(args.follow_symlinks);
                assert!(args.skip_hidden);
                assert_eq!(args.io_threads, Some(8));
                assert!(args.paranoid);
                assert!(args.permanent);
                assert!(args.yes);
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_no_color_env() {
        // Use a lock if we had one, but since we don't, we'll just be careful.
        // Note: This test may fail if run in parallel with other CLI tests.
        std::env::set_var("NO_COLOR", "true");
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/path"]).unwrap();
        assert!(cli.no_color);
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_cli_invalid_subcommand() {
        let result = Cli::try_parse_from(["rustdupe", "invalid", "/path"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_missing_path() {
        let result = Cli::try_parse_from(["rustdupe", "scan"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_version_flag() {
        let result = Cli::try_parse_from(["rustdupe", "--version"]);
        assert!(result.is_err()); // clap exits on --version
    }

    #[test]
    fn test_cli_parse_scan_session_flags() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "/path",
            "--save-session",
            "session.json",
            "--reference",
            "/ref1",
            "--reference",
            "/ref2",
            "--dry-run",
        ])
        .unwrap();

        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.paths, vec![PathBuf::from("/path")]);
                assert_eq!(args.save_session, Some(PathBuf::from("session.json")));
                assert_eq!(
                    args.reference_paths,
                    vec![PathBuf::from("/ref1"), PathBuf::from("/ref2")]
                );
                assert!(args.dry_run);
            }
            _ => panic!("Expected Scan command"),
        }

        let cli = Cli::try_parse_from(["rustdupe", "scan", "/path", "--analyze-only"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert!(args.dry_run);
            }
            _ => panic!("Expected Scan command"),
        }

        let cli =
            Cli::try_parse_from(["rustdupe", "scan", "--load-session", "session.json"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert!(args.paths.is_empty());
                assert_eq!(args.load_session, Some(PathBuf::from("session.json")));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_load_subcommand() {
        let cli =
            Cli::try_parse_from(["rustdupe", "load", "session.json", "--output", "json"]).unwrap();
        match cli.command {
            Commands::Load(args) => {
                assert_eq!(args.path, PathBuf::from("session.json"));
                assert_eq!(args.output, Some(OutputFormat::Json));
            }
            _ => panic!("Expected Load command"),
        }
    }

    #[test]
    fn test_cli_parse_cache_flags() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "/path",
            "--cache",
            "mycache.db",
            "--clear-cache",
        ])
        .unwrap();

        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.cache, Some(PathBuf::from("mycache.db")));
                assert!(args.clear_cache);
                assert!(!args.no_cache);
            }
            _ => panic!("Expected Scan command"),
        }

        let cli = Cli::try_parse_from(["rustdupe", "scan", "/path", "--no-cache"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert!(args.no_cache);
                assert!(args.cache.is_none());
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_keybinding_profile_not_specified() {
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/path"]).unwrap();
        // When not specified, keybinding_profile should be None
        assert!(cli.keybinding_profile.is_none());
    }

    #[test]
    fn test_cli_keybinding_profile_universal() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "--keybinding-profile",
            "universal",
            "scan",
            "/path",
        ])
        .unwrap();
        assert_eq!(cli.keybinding_profile, Some(KeybindingProfile::Universal));
    }

    #[test]
    fn test_cli_keybinding_profile_vim() {
        let cli = Cli::try_parse_from(["rustdupe", "--keybinding-profile", "vim", "scan", "/path"])
            .unwrap();
        assert_eq!(cli.keybinding_profile, Some(KeybindingProfile::Vim));
    }

    #[test]
    fn test_cli_keybinding_profile_standard() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "--keybinding-profile",
            "standard",
            "scan",
            "/path",
        ])
        .unwrap();
        assert_eq!(cli.keybinding_profile, Some(KeybindingProfile::Standard));
    }

    #[test]
    fn test_cli_keybinding_profile_emacs() {
        let cli =
            Cli::try_parse_from(["rustdupe", "--keybinding-profile", "emacs", "scan", "/path"])
                .unwrap();
        assert_eq!(cli.keybinding_profile, Some(KeybindingProfile::Emacs));
    }

    #[test]
    fn test_cli_keybinding_profile_alias() {
        // Test the --keys alias
        let cli = Cli::try_parse_from(["rustdupe", "--keys", "vim", "scan", "/path"]).unwrap();
        assert_eq!(cli.keybinding_profile, Some(KeybindingProfile::Vim));
    }

    #[test]
    fn test_cli_keybinding_profile_invalid() {
        let result = Cli::try_parse_from([
            "rustdupe",
            "--keybinding-profile",
            "invalid",
            "scan",
            "/path",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_keybinding_profile_global_flag() {
        // Test that keybinding profile works as a global flag (before subcommand)
        let cli = Cli::try_parse_from([
            "rustdupe",
            "--keybinding-profile",
            "vim",
            "load",
            "session.json",
        ])
        .unwrap();
        assert_eq!(cli.keybinding_profile, Some(KeybindingProfile::Vim));
    }

    #[test]
    fn test_cli_accessible_flag_not_set() {
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/path"]).unwrap();
        assert!(!cli.accessible);
    }

    #[test]
    fn test_cli_accessible_flag_set() {
        let cli = Cli::try_parse_from(["rustdupe", "--accessible", "scan", "/path"]).unwrap();
        assert!(cli.accessible);
    }

    #[test]
    fn test_cli_accessible_flag_with_load() {
        let cli =
            Cli::try_parse_from(["rustdupe", "--accessible", "load", "session.json"]).unwrap();
        assert!(cli.accessible);
    }

    #[test]
    fn test_cli_accessible_combined_with_other_flags() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "--accessible",
            "--no-color",
            "-v",
            "scan",
            "/path",
        ])
        .unwrap();
        assert!(cli.accessible);
        assert!(cli.no_color);
        assert_eq!(cli.verbose, 1);
    }

    #[test]
    fn test_cli_parse_scan_multiple_paths() {
        let cli =
            Cli::try_parse_from(["rustdupe", "scan", "/path/1", "/path/2", "/path/3"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.paths.len(), 3);
                assert_eq!(args.paths[0], PathBuf::from("/path/1"));
                assert_eq!(args.paths[1], PathBuf::from("/path/2"));
                assert_eq!(args.paths[2], PathBuf::from("/path/3"));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_two_paths() {
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/downloads", "/documents"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(
                    args.paths,
                    vec![PathBuf::from("/downloads"), PathBuf::from("/documents"),]
                );
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_multiple_paths_with_options() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "-v",
            "scan",
            "/path/1",
            "/path/2",
            "--output",
            "json",
            "--min-size",
            "1MB",
        ])
        .unwrap();
        assert_eq!(cli.verbose, 1);
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.paths.len(), 2);
                assert_eq!(args.paths[0], PathBuf::from("/path/1"));
                assert_eq!(args.paths[1], PathBuf::from("/path/2"));
                assert_eq!(args.output, Some(OutputFormat::Json));
                assert_eq!(args.min_size, Some(1_000_000));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_zero_paths_error() {
        // Zero paths without --load-session should fail
        let result = Cli::try_parse_from(["rustdupe", "scan"]);
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        // The error should mention the required argument
        assert!(err_str.contains("PATH") || err_str.contains("required"));
    }

    #[test]
    fn test_cli_parse_scan_single_path_backward_compat() {
        // Single path should still work (backward compatibility)
        let cli = Cli::try_parse_from(["rustdupe", "scan", "/single/path"]).unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.paths.len(), 1);
                assert_eq!(args.paths[0], PathBuf::from("/single/path"));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    // ========================================================================
    // Directory Group Tests
    // ========================================================================

    #[test]
    fn test_parse_group_valid() {
        let group = parse_group("photos=/home/user/photos").unwrap();
        assert_eq!(group.name, "photos");
        assert_eq!(group.path, PathBuf::from("/home/user/photos"));
    }

    #[test]
    fn test_parse_group_with_spaces() {
        let group = parse_group("  photos = /home/user/photos  ").unwrap();
        assert_eq!(group.name, "photos");
        assert_eq!(group.path, PathBuf::from("/home/user/photos"));
    }

    #[test]
    fn test_parse_group_with_underscores() {
        let group = parse_group("my_photos=/path").unwrap();
        assert_eq!(group.name, "my_photos");
    }

    #[test]
    fn test_parse_group_with_hyphens() {
        let group = parse_group("my-photos=/path").unwrap();
        assert_eq!(group.name, "my-photos");
    }

    #[test]
    fn test_parse_group_with_numbers() {
        let group = parse_group("photos2024=/path").unwrap();
        assert_eq!(group.name, "photos2024");
    }

    #[test]
    fn test_parse_group_missing_equals() {
        let result = parse_group("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected NAME=PATH"));
    }

    #[test]
    fn test_parse_group_empty_name() {
        let result = parse_group("=/path");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_parse_group_empty_path() {
        let result = parse_group("name=");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_parse_group_invalid_name_chars() {
        let result = parse_group("my photos=/path");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid group name"));
    }

    #[test]
    fn test_cli_parse_scan_single_group() {
        let cli = Cli::try_parse_from(["rustdupe", "scan", "--group", "photos=/Photos", "/path"])
            .unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.groups.len(), 1);
                assert_eq!(args.groups[0].name, "photos");
                assert_eq!(args.groups[0].path, PathBuf::from("/Photos"));
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_multiple_groups() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "--group",
            "photos=/Photos",
            "--group",
            "docs=/Documents",
            "/path",
        ])
        .unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.groups.len(), 2);
                assert_eq!(args.groups[0].name, "photos");
                assert_eq!(args.groups[1].name, "docs");
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_scan_groups_with_multiple_paths() {
        let cli = Cli::try_parse_from([
            "rustdupe",
            "scan",
            "--group",
            "photos=/Photos",
            "--group",
            "downloads=/Downloads",
            "/Photos",
            "/Downloads",
        ])
        .unwrap();
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.groups.len(), 2);
                assert_eq!(args.paths.len(), 2);
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_parse_profile_flag() {
        let cli =
            Cli::try_parse_from(["rustdupe", "--profile", "photos", "scan", "/path"]).unwrap();
        assert_eq!(cli.profile, Some("photos".to_string()));
    }

    #[test]
    fn test_cli_parse_list_profiles_flag() {
        let cli = Cli::try_parse_from(["rustdupe", "--list-profiles", "scan", "/path"]).unwrap();
        assert!(cli.list_profiles);
    }

    #[test]
    fn test_build_group_map_empty() {
        let groups: Vec<DirectoryGroup> = vec![];
        let map = build_group_map(&groups).unwrap();
        assert!(map.is_empty());
    }
}
