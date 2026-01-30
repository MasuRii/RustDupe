//! RustDupe - Smart Duplicate File Finder
//!
//! A cross-platform Rust CLI application for finding and managing duplicate files
//! using content hashing (BLAKE3), with an interactive TUI for review and safe deletion.

pub mod actions;
pub mod duplicates;
pub mod scanner;
pub mod tui;
