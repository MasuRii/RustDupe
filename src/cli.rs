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
use std::path::PathBuf;

use crate::tui::keybindings::KeybindingProfile;

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
    #[arg(long, global = true, env = "NO_COLOR")]
    pub no_color: bool,

    /// TUI theme (light, dark, auto)
    #[arg(long, value_enum, default_value = "auto", global = true)]
    pub theme: ThemeArg,

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
    /// Directory path to scan for duplicates
    #[arg(value_name = "PATH", required_unless_present = "load_session")]
    pub path: Option<PathBuf>,

    /// Load a previously saved session instead of scanning
    #[arg(
        long,
        value_name = "SESSION_FILE",
        conflicts_with = "path",
        help_heading = "Scanning Options"
    )]
    pub load_session: Option<PathBuf>,

    /// Save scan results to a session file
    #[arg(long, value_name = "PATH", help_heading = "Output Options")]
    pub save_session: Option<PathBuf>,

    /// Output format (tui for interactive, json/csv for scripting, session for persistence, html for report, script for deletion)
    #[arg(
        short,
        long,
        value_enum,
        default_value = "tui",
        help_heading = "Output Options"
    )]
    pub output: OutputFormat,

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
    #[arg(long, help_heading = "Scanning Options")]
    pub follow_symlinks: bool,

    /// Skip hidden files and directories (starting with .)
    #[arg(long, help_heading = "Scanning Options")]
    pub skip_hidden: bool,

    /// Number of I/O threads for hashing (default: 4)
    ///
    /// Lower values reduce disk thrashing on HDDs.
    #[arg(
        long,
        value_name = "N",
        default_value = "4",
        help_heading = "Scanning Options"
    )]
    pub io_threads: usize,

    /// Enable paranoid mode: byte-by-byte verification after hash match
    ///
    /// Slower but guarantees no hash collisions.
    #[arg(long, help_heading = "Scanning Options")]
    pub paranoid: bool,

    /// Use permanent deletion instead of moving to trash
    ///
    /// Warning: Files cannot be recovered after permanent deletion.
    #[arg(long, help_heading = "Safety & Deletion Options")]
    pub permanent: bool,

    /// Skip confirmation prompts (required with --permanent in non-interactive mode)
    #[arg(short = 'y', long, help_heading = "Safety & Deletion Options")]
    pub yes: bool,

    /// Path to the hash cache database
    ///
    /// If not specified, a default platform-specific path is used.
    #[arg(long, value_name = "PATH", help_heading = "Cache Options")]
    pub cache: Option<PathBuf>,

    /// Disable hash caching
    #[arg(long, conflicts_with = "cache", help_heading = "Cache Options")]
    pub no_cache: bool,

    /// Clear the hash cache before scanning
    #[arg(long, help_heading = "Cache Options")]
    pub clear_cache: bool,

    /// Do not perform any deletions (read-only mode)
    #[arg(
        long,
        alias = "analyze-only",
        help_heading = "Safety & Deletion Options"
    )]
    pub dry_run: bool,

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
}

/// Arguments for the load subcommand.
#[derive(Debug, Args)]
pub struct LoadArgs {
    /// Session file to load
    #[arg(value_name = "SESSION_FILE")]
    pub path: PathBuf,

    /// Output format (tui for interactive, json/csv for scripting, html for report, script for deletion)
    #[arg(
        short,
        long,
        value_enum,
        default_value = "tui",
        help_heading = "Output Options"
    )]
    pub output: OutputFormat,

    /// Write output to a file instead of stdout
    #[arg(long, value_name = "PATH", help_heading = "Output Options")]
    pub output_file: Option<PathBuf>,

    /// Type of deletion script to generate
    #[arg(long, value_enum, value_name = "TYPE", help_heading = "Output Options")]
    pub script_type: Option<ScriptTypeArg>,

    /// Do not perform any deletions (read-only mode)
    #[arg(long, alias = "analyze-only", help_heading = "Safety Options")]
    pub dry_run: bool,
}

/// Output format for scan results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Interactive terminal user interface
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScriptTypeArg {
    /// POSIX-compliant shell script (sh/bash/zsh)
    Posix,
    /// Windows PowerShell script
    Powershell,
}

/// File type categories for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
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
                assert_eq!(args.path, Some(PathBuf::from("/some/path")));
                assert_eq!(args.output, OutputFormat::Tui);
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
                assert_eq!(args.output, OutputFormat::Json);
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
                assert_eq!(args.output, OutputFormat::Script);
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
                assert_eq!(args.output, OutputFormat::Script);
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
                assert_eq!(args.output, OutputFormat::Csv);
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
                assert_eq!(args.io_threads, 8);
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
                assert_eq!(args.path, Some(PathBuf::from("/path")));
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
                assert_eq!(args.path, None);
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
                assert_eq!(args.output, OutputFormat::Json);
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
}
