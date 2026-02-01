//! RustDupe - Smart Duplicate File Finder
//!
//! A cross-platform Rust CLI application for finding and managing duplicate files
//! using content hashing (BLAKE3), with an interactive TUI for review and safe deletion.
//!
//! # Architecture
//!
//! The crate is organized into the following modules:
//!
//! - [`cli`]: Command-line argument parsing and validation
//! - [`logging`]: Logging infrastructure and initialization
//! - [`signal`]: Signal handling for graceful shutdown
//! - [`scanner`]: Directory traversal and file hashing
//! - [`duplicates`]: Duplicate detection engine
//! - [`tui`]: Interactive terminal user interface
//! - [`actions`]: File operations (delete, preview)
//! - [`cache`]: Persistent hash caching for faster rescans
//! - [`output`]: Output formatters (JSON, CSV)

// =============================================================================
// Clippy Lint Configuration
// =============================================================================
//
// We use Clippy's default warnings-as-errors (-D warnings) for CI quality gates.
// Pedantic lints are NOT enabled project-wide because they generate too many
// false positives for this codebase (e.g., doc_markdown, cast_precision_loss).
//
// Threshold configuration is in clippy.toml:
// - too-many-arguments-threshold = 7
// - too-many-lines-threshold = 150
// - cognitive-complexity-threshold = 25
// - msrv = "1.75.0"
//
// To run with pedantic lints for review (not CI):
//   cargo clippy -- -W clippy::pedantic
//
// Allow specific lints with documented justification:

// `module_name_repetitions`: We prefer explicit names like `DeleteError` in `delete` module
// over generic names that lose context when imported.
#![allow(clippy::module_name_repetitions)]

pub mod actions;
pub mod cache;
pub mod cli;
pub mod duplicates;
pub mod logging;
pub mod output;
pub mod progress;
pub mod scanner;
pub mod session;
pub mod signal;
pub mod tui;
