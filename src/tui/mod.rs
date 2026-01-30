//! Terminal User Interface module.
//!
//! This module provides the interactive TUI for reviewing and
//! managing duplicate files using ratatui with crossterm backend.
//!
//! # Overview
//!
//! The TUI module consists of:
//! - [`app`]: Application state management (modes, navigation, selection)
//! - [`events`]: Keyboard event handling (TODO: Task 3.4.2)
//! - [`ui`]: Ratatui rendering (TODO: Task 3.4.3)
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
//! use rustdupe::tui::app::{App, AppMode, Action};
//! use rustdupe::duplicates::DuplicateGroup;
//! use std::path::PathBuf;
//!
//! // Create app with duplicate groups
//! let groups = vec![
//!     DuplicateGroup::new(
//!         [0u8; 32],
//!         1000,
//!         vec![PathBuf::from("/a.txt"), PathBuf::from("/b.txt")],
//!     ),
//! ];
//! let mut app = App::with_groups(groups);
//!
//! // Handle user actions
//! app.handle_action(Action::NavigateDown);
//! app.handle_action(Action::ToggleSelect);
//!
//! // Check state
//! assert_eq!(app.file_index(), 1);
//! assert!(app.has_selections());
//! ```

pub mod app;
pub mod events;
pub mod ui;

// Re-export commonly used types
pub use app::{Action, App, AppMode, ScanProgress};
pub use events::{EventError, EventHandler};
pub use ui::{format_size, render, truncate_path, truncate_string};
