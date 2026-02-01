//! Terminal User Interface module.
//!
//! This module provides the interactive TUI for reviewing and
//! managing duplicate files using ratatui with crossterm backend.
//!
//! # Overview
//!
//! The TUI module consists of:
//! - [`app`]: Application state management (modes, navigation, selection)
//! - [`events`]: Keyboard event handling
//! - [`ui`]: Ratatui rendering
//! - [`run_tui`]: Main loop that coordinates everything
//!
//! # Architecture
//!
//! The TUI follows a unidirectional data flow:
//! 1. Events are captured from the terminal (crossterm)
//! 2. Events are translated to Actions
//! 3. Actions modify the App state
//! 4. The UI renders based on the current App state
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::tui::{run_tui, App};
//! use rustdupe::duplicates::DuplicateGroup;
//! use std::path::PathBuf;
//!
//! // Create app with duplicate groups
//! let groups = vec![
//!     DuplicateGroup::new(
//!         [0u8; 32],
//!         1000,
//!         vec![
//!             rustdupe::scanner::FileEntry::new(PathBuf::from("/a.txt"), 1000, std::time::SystemTime::now()),
//!             rustdupe::scanner::FileEntry::new(PathBuf::from("/b.txt"), 1000, std::time::SystemTime::now()),
//!         ],
//!         vec![],
//!     ),
//! ];
//! let mut app = App::with_groups(groups);
//!
//! // Run the TUI (this takes over the terminal)
//! // run_tui(&mut app, None).unwrap();
//! ```

pub mod app;
pub mod events;
mod run;
pub mod ui;

// Re-export commonly used types
pub use app::{Action, App, AppMode, ScanProgress};
pub use events::{EventError, EventHandler};
pub use run::{run_tui, TuiError};
pub use ui::{format_size, render, truncate_path, truncate_string};
