//! Logging infrastructure for RustDupe.
//!
//! This module provides structured logging using the `log` facade and `env_logger` backend.
//! Log levels are determined by (in priority order):
//!
//! 1. `RUST_LOG` environment variable (if set)
//! 2. CLI flags: `--quiet` (error only) or `--verbose` (debug/trace)
//! 3. Default: info level
//!
//! # Build-specific Formatting
//!
//! - **Debug builds**: Include timestamp, level, and module path for detailed debugging
//! - **Release builds**: Compact format with level and message only for cleaner output
//!
//! # Example
//!
//! ```rust,no_run
//! use rustdupe::logging::init_logging;
//!
//! // Initialize with default (info) level
//! init_logging(0, false);
//!
//! // Initialize with verbose mode (-v)
//! init_logging(1, false);
//!
//! // Initialize with trace mode (-vv)
//! init_logging(2, false);
//!
//! // Initialize with quiet mode (errors only)
//! init_logging(0, true);
//! ```

use env_logger::Builder;
use log::LevelFilter;
use std::env;
use std::io::Write;

/// Initialize the logging subsystem based on CLI verbosity flags.
///
/// This function should be called once at the start of the application,
/// before any logging calls are made.
///
/// # Priority
///
/// 1. If `RUST_LOG` environment variable is set, it takes precedence
/// 2. If `quiet` is true: Error level only
/// 3. If `verbose >= 2`: Trace level
/// 4. If `verbose == 1`: Debug level
/// 5. Default: Info level
///
/// # Arguments
///
/// * `verbose` - Verbosity count from CLI (0=normal, 1=debug, 2+=trace)
/// * `quiet` - If true, only show errors (overridden by RUST_LOG)
///
/// # Panics
///
/// This function will panic if called more than once, as `env_logger`
/// can only be initialized once per process.
///
/// # Example
///
/// ```rust,no_run
/// use rustdupe::logging::init_logging;
///
/// // Normal usage with CLI flags
/// let verbose = 0;
/// let quiet = false;
/// init_logging(verbose, quiet);
///
/// log::info!("Application started");
/// log::debug!("Debug info here");
/// ```
pub fn init_logging(verbose: u8, quiet: bool) {
    // Check if RUST_LOG is set - if so, use env_logger's default behavior
    let use_env = env::var("RUST_LOG").is_ok();

    let mut builder = Builder::new();

    if use_env {
        // Use RUST_LOG environment variable
        builder.parse_default_env();
        log::debug!(
            "Logging initialized from RUST_LOG environment variable: {:?}",
            env::var("RUST_LOG").ok()
        );
    } else {
        // Determine level from CLI flags
        let level = determine_level(verbose, quiet);
        builder.filter_level(level);
    }

    // Configure format based on build type
    configure_format(&mut builder, verbose);

    // Initialize the logger
    builder.init();

    // Log initialization message (only if not using RUST_LOG, as we already logged above)
    if !use_env {
        let level = determine_level(verbose, quiet);
        log::debug!("Logging initialized at level: {:?}", level);
    }
}

/// Determine the log level from CLI flags.
///
/// # Arguments
///
/// * `verbose` - Verbosity count (0=info, 1=debug, 2+=trace)
/// * `quiet` - If true, use error level
///
/// # Returns
///
/// The appropriate `LevelFilter` based on the flags.
fn determine_level(verbose: u8, quiet: bool) -> LevelFilter {
    if quiet {
        LevelFilter::Error
    } else {
        match verbose {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

/// Configure the log format based on build type and verbosity.
///
/// - Debug builds: timestamp, level, module path (for detailed debugging)
/// - Release builds: compact format (level + message only)
fn configure_format(builder: &mut Builder, verbose: u8) {
    // In debug builds, include more information
    #[cfg(debug_assertions)]
    {
        builder.format(move |buf, record| {
            let timestamp = buf.timestamp_seconds();
            let level = record.level();
            let level_style = buf.default_level_style(level);

            if verbose >= 2 {
                // Trace mode: include full module path
                writeln!(
                    buf,
                    "{} {level_style}{:<5}{level_style:#} [{}] {}",
                    timestamp,
                    level,
                    record.module_path().unwrap_or("unknown"),
                    record.args()
                )
            } else if verbose >= 1 {
                // Debug mode: include module path
                writeln!(
                    buf,
                    "{} {level_style}{:<5}{level_style:#} [{}] {}",
                    timestamp,
                    level,
                    record.module_path().unwrap_or("unknown"),
                    record.args()
                )
            } else {
                // Info mode: timestamp and level
                writeln!(
                    buf,
                    "{} {level_style}{:<5}{level_style:#} {}",
                    timestamp,
                    level,
                    record.args()
                )
            }
        });
    }

    // In release builds, use compact format
    #[cfg(not(debug_assertions))]
    {
        let _ = verbose; // Suppress unused variable warning in release
        builder.format(|buf, record| {
            let level = record.level();
            let level_style = buf.default_level_style(level);
            writeln!(
                buf,
                "{level_style}{:<5}{level_style:#} {}",
                level,
                record.args()
            )
        });
    }
}

/// Get the current log level as a string.
///
/// Useful for displaying the current logging configuration to users.
///
/// # Returns
///
/// A string representation of the maximum log level.
pub fn current_level_name() -> &'static str {
    match log::max_level() {
        LevelFilter::Off => "off",
        LevelFilter::Error => "error",
        LevelFilter::Warn => "warn",
        LevelFilter::Info => "info",
        LevelFilter::Debug => "debug",
        LevelFilter::Trace => "trace",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_level_default() {
        assert_eq!(determine_level(0, false), LevelFilter::Info);
    }

    #[test]
    fn test_determine_level_verbose() {
        assert_eq!(determine_level(1, false), LevelFilter::Debug);
    }

    #[test]
    fn test_determine_level_trace() {
        assert_eq!(determine_level(2, false), LevelFilter::Trace);
        assert_eq!(determine_level(3, false), LevelFilter::Trace);
    }

    #[test]
    fn test_determine_level_quiet() {
        assert_eq!(determine_level(0, true), LevelFilter::Error);
    }

    #[test]
    fn test_determine_level_quiet_overrides_verbose() {
        // quiet takes precedence over verbose
        assert_eq!(determine_level(2, true), LevelFilter::Error);
    }

    #[test]
    fn test_current_level_name_values() {
        // This test verifies the function doesn't panic
        // The actual level depends on whether init_logging was called
        let name = current_level_name();
        assert!(
            ["off", "error", "warn", "info", "debug", "trace"].contains(&name),
            "Unexpected level name: {}",
            name
        );
    }
}
