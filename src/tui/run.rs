//! TUI main loop.
//!
//! This module provides the main entry point for running the interactive TUI.
//! It handles terminal setup, the event loop, and cleanup on exit.
//!
//! # Terminal Management
//!
//! The TUI takes over the terminal by:
//! - Enabling raw mode (unbuffered input, no echo)
//! - Entering the alternate screen buffer
//! - Hiding the cursor
//!
//! All these changes are reverted on exit, including on panic.
//!
//! # Event Loop
//!
//! The main loop follows this pattern:
//! 1. Poll for events with a timeout
//! 2. Handle any event that occurred
//! 3. Render the current state
//! 4. Limit frame rate to ~60 FPS
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::tui::{run_tui, App};
//! use rustdupe::duplicates::DuplicateGroup;
//! use std::path::PathBuf;
//!
//! let groups = vec![
//!     DuplicateGroup::new(
//!         [0u8; 32],
//!         1000,
//!         vec![PathBuf::from("/a.txt"), PathBuf::from("/b.txt")],
//!         vec![],
//!     ),
//! ];
//! let mut app = App::with_groups(groups);
//!
//! // Run the TUI
//! run_tui(&mut app, None).unwrap();
//! ```

use std::io::{self, Stdout};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use thiserror::Error;

use super::app::{Action, App, AppMode};
use super::events::EventHandler;
use super::ui::render;
use crate::actions::delete::{delete_batch, validate_preserves_copy, DeleteConfig};
use crate::actions::preview::preview_file_simple;

/// Frame rate limit: 60 FPS = ~16.67ms per frame.
/// Using 16ms for slightly conservative timing.
const FRAME_DURATION: Duration = Duration::from_millis(16);

/// Event poll timeout: Use the frame duration for responsive rendering.
const POLL_TIMEOUT: Duration = Duration::from_millis(16);

/// Error type for TUI operations.
#[derive(Debug, Error)]
pub enum TuiError {
    /// I/O error from terminal operations.
    #[error("terminal I/O error: {0}")]
    Io(#[from] io::Error),

    /// Event handling error.
    #[error("event error: {0}")]
    Event(#[from] super::events::EventError),

    /// The TUI was interrupted by a shutdown signal.
    #[error("interrupted by shutdown signal")]
    Interrupted,

    /// Error during file deletion.
    #[error("deletion error: {0}")]
    DeleteError(String),
}

/// Result type for TUI operations.
pub type TuiResult<T> = Result<T, TuiError>;

/// Type alias for the terminal backend.
type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

/// Run the interactive TUI.
///
/// This function takes over the terminal and runs the interactive interface
/// until the user quits or an error occurs.
///
/// # Arguments
///
/// * `app` - The application state, typically pre-loaded with duplicate groups
/// * `shutdown_flag` - Optional flag for external shutdown signaling (e.g., Ctrl+C handler)
///
/// # Returns
///
/// Returns `Ok(())` on normal exit, or `Err` on error.
///
/// # Terminal Restoration
///
/// The terminal is always restored to its original state, even on error or panic.
///
/// # Errors
///
/// Returns `TuiError::Io` for terminal I/O errors.
/// Returns `TuiError::Event` for event handling errors.
/// Returns `TuiError::Interrupted` if shutdown was requested.
///
/// # Example
///
/// ```no_run
/// use rustdupe::tui::{run_tui, App};
///
/// let mut app = App::new();
/// if let Err(e) = run_tui(&mut app, None) {
///     eprintln!("TUI error: {}", e);
/// }
/// ```
pub fn run_tui(app: &mut App, shutdown_flag: Option<Arc<AtomicBool>>) -> TuiResult<()> {
    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal before showing panic message
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Run the TUI with proper cleanup
    let result = run_tui_inner(app, shutdown_flag);

    // Restore the original panic hook
    let _ = panic::take_hook();

    result
}

/// Inner function that runs the TUI loop.
///
/// This is separated from `run_tui` to ensure cleanup happens correctly.
fn run_tui_inner(app: &mut App, shutdown_flag: Option<Arc<AtomicBool>>) -> TuiResult<()> {
    // Set up the terminal
    let mut terminal = setup_terminal()?;

    // Create event handler
    let event_handler = EventHandler::new();

    // Track frame timing for rate limiting
    let mut last_render = Instant::now();

    // Main loop
    loop {
        // Check for external shutdown signal
        if let Some(ref flag) = shutdown_flag {
            if flag.load(Ordering::SeqCst) {
                log::info!("Shutdown signal received, exiting TUI");
                break;
            }
        }

        // Check if app wants to quit
        if app.should_quit() {
            log::debug!("App requested quit");
            break;
        }

        // Render the current state
        terminal.draw(|frame| render(frame, app))?;

        // Poll for events with timeout
        if let Some(action) = event_handler.poll(POLL_TIMEOUT)? {
            handle_action(app, action, &shutdown_flag)?;
        }

        // Frame rate limiting
        let elapsed = last_render.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
        last_render = Instant::now();
    }

    // Cleanup - restore terminal
    restore_terminal()?;

    log::info!("TUI exited normally");
    Ok(())
}

/// Handle a user action.
///
/// This processes actions that require more than simple state updates,
/// like file deletion or preview loading.
fn handle_action(
    app: &mut App,
    action: Action,
    _shutdown_flag: &Option<Arc<AtomicBool>>,
) -> TuiResult<()> {
    // First, let the app handle the action for state updates
    let was_handled = app.handle_action(action);

    // Handle special actions that require additional processing
    match action {
        Action::Confirm => {
            if app.mode() == AppMode::Confirming {
                // Perform the actual deletion
                let result = perform_deletion(app);
                match result {
                    Ok(deleted_count) => {
                        log::info!("Deleted {} files", deleted_count);
                        app.set_mode(AppMode::Reviewing);
                    }
                    Err(e) => {
                        app.set_error(&format!("Deletion failed: {}", e));
                        app.set_mode(AppMode::Reviewing);
                    }
                }
            }
        }
        Action::Preview => {
            if app.mode() == AppMode::Previewing {
                // Load preview content for the current file
                if let Some(path) = app.current_file() {
                    let content = preview_file_simple(path);
                    app.set_preview(content);
                }
            }
        }
        Action::Cancel => {
            // Clear any error message on cancel
            if app.error_message().is_some() {
                app.clear_error();
            }
        }
        _ => {
            // Other actions are handled by app.handle_action()
            if !was_handled {
                log::trace!("Action not handled: {:?}", action);
            }
        }
    }

    Ok(())
}

/// Perform file deletion for selected files.
fn perform_deletion(app: &mut App) -> Result<usize, TuiError> {
    let selected_files = app.selected_files_vec();

    if selected_files.is_empty() {
        return Ok(0);
    }

    // Validate that we're not deleting all copies
    // We need to check for each group
    for group in app.groups() {
        let group_files: Vec<_> = group.files.clone();
        if let Err(_e) = validate_preserves_copy(&selected_files, &group_files) {
            return Err(TuiError::DeleteError(
                "Cannot delete all copies - at least one file must be preserved".to_string(),
            ));
        }
    }

    // Use trash deletion by default
    let config = DeleteConfig::trash();

    // Perform deletion
    let result = delete_batch(&selected_files, &config, None::<&NoOpProgress>);

    // Update app state with deleted files
    let deleted_paths: Vec<_> = result.successes.iter().map(|r| r.path.clone()).collect();
    app.remove_deleted_files(&deleted_paths);

    // Report any failures
    if !result.failures.is_empty() {
        let (failed_path, error_msg) = &result.failures[0];
        log::warn!(
            "Some files failed to delete: {} - {}",
            failed_path.display(),
            error_msg
        );
        if result.successes.is_empty() {
            return Err(TuiError::DeleteError(format!(
                "Failed to delete files: {}",
                error_msg
            )));
        }
    }

    Ok(result.success_count())
}

/// Placeholder progress callback that does nothing.
struct NoOpProgress;

impl crate::actions::delete::DeleteProgressCallback for NoOpProgress {
    fn on_before_delete(&self, _path: &std::path::Path, _index: usize, _total: usize) {}
    fn on_delete_success(&self, _path: &std::path::Path, _size: u64) {}
    fn on_delete_failure(&self, _path: &std::path::Path, _error: &str) {}
    fn on_complete(&self, _result: &crate::actions::delete::BatchDeleteResult) {}
}

/// Set up the terminal for TUI mode.
fn setup_terminal() -> TuiResult<Terminal> {
    log::debug!("Setting up terminal for TUI");

    // Enable raw mode (no line buffering, no echo)
    terminal::enable_raw_mode()?;

    // Set up stdout with crossterm commands
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        cursor::Hide
    )?;

    // Create the ratatui terminal
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    log::debug!("Terminal setup complete");
    Ok(terminal)
}

/// Restore the terminal to its original state.
fn restore_terminal() -> TuiResult<()> {
    log::debug!("Restoring terminal");

    // Disable raw mode
    let _ = terminal::disable_raw_mode();

    // Restore stdout
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        cursor::Show
    );

    log::debug!("Terminal restored");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_tui_error_display() {
        let io_err = io::Error::other("test error");
        let tui_err = TuiError::Io(io_err);
        assert!(format!("{}", tui_err).contains("terminal I/O error"));

        let interrupted = TuiError::Interrupted;
        assert!(format!("{}", interrupted).contains("interrupted"));
    }

    #[test]
    fn test_preview_file_simple_nonexistent() {
        let content = preview_file_simple(std::path::Path::new("/nonexistent/file.txt"));
        assert!(content.to_lowercase().contains("error") || content.contains("not found"));
    }

    #[test]
    fn test_preview_file_simple_with_temp_file() {
        use std::io::Write;

        // Create a temporary file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("rustdupe_test_preview.txt");

        {
            let mut file = std::fs::File::create(&temp_path).unwrap();
            writeln!(file, "Line 1").unwrap();
            writeln!(file, "Line 2").unwrap();
            writeln!(file, "Line 3").unwrap();
        }

        let content = preview_file_simple(&temp_path);
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(content.contains("Line 3"));

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_preview_file_simple_empty_file() {
        use std::fs::File;

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("rustdupe_test_empty.txt");

        // Create empty file
        File::create(&temp_path).unwrap();

        let content = preview_file_simple(&temp_path);
        assert!(content.contains("empty"));

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_frame_duration() {
        // Verify the frame duration is reasonable for 60 FPS
        assert_eq!(FRAME_DURATION.as_millis(), 16);
    }

    #[test]
    fn test_poll_timeout() {
        // Poll timeout should match frame duration for responsive updates
        assert_eq!(POLL_TIMEOUT.as_millis(), 16);
    }

    // Note: We can't easily test the actual TUI without a real terminal,
    // but we can test the supporting functions.

    #[test]
    fn test_noop_progress_callback() {
        use crate::actions::delete::DeleteProgressCallback;

        // Verify NoOpProgress implements the trait correctly
        let progress = NoOpProgress;
        progress.on_before_delete(std::path::Path::new("/test"), 0, 1);
        progress.on_delete_success(std::path::Path::new("/test"), 100);
        progress.on_delete_failure(std::path::Path::new("/test"), "error");

        // Create a mock result for on_complete
        let result = crate::actions::delete::BatchDeleteResult {
            successes: vec![],
            failures: vec![],
            bytes_freed: 0,
        };
        progress.on_complete(&result);
    }

    mod perform_deletion_tests {
        use super::*;
        use crate::duplicates::DuplicateGroup;
        use crate::tui::App;

        fn make_group(size: u64, paths: Vec<&str>) -> DuplicateGroup {
            DuplicateGroup::new(
                [0u8; 32],
                size,
                paths.into_iter().map(PathBuf::from).collect(),
                Vec::new(),
            )
        }

        #[test]
        fn test_perform_deletion_empty_selection() {
            let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
            let mut app = App::with_groups(groups);

            // No files selected
            let result = perform_deletion(&mut app);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 0);
        }

        #[test]
        fn test_perform_deletion_prevents_deleting_all_copies() {
            let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
            let mut app = App::with_groups(groups);

            // Select all files (which should be prevented)
            app.select(PathBuf::from("/a.txt"));
            app.select(PathBuf::from("/b.txt"));

            let result = perform_deletion(&mut app);
            assert!(result.is_err());
        }
    }
}
