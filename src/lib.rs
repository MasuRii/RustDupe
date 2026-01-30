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
//! - [`scanner`]: Directory traversal and file hashing
//! - [`duplicates`]: Duplicate detection engine
//! - [`tui`]: Interactive terminal user interface
//! - [`actions`]: File operations (delete, preview)

pub mod actions;
pub mod cli;
pub mod duplicates;
pub mod logging;
pub mod scanner;
pub mod tui;
