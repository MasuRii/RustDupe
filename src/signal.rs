//! Signal handling for graceful shutdown.
//!
//! This module provides centralized Ctrl+C handling for the RustDupe application.
//! It uses an `AtomicBool` flag that can be shared across threads to signal when
//! shutdown has been requested.
//!
//! # Usage
//!
//! ```rust,no_run
//! use rustdupe::signal::{ShutdownHandler, install_handler};
//!
//! // Create and install the handler
//! let handler = install_handler().expect("Failed to install signal handler");
//!
//! // Check if shutdown was requested anywhere in your code
//! if handler.is_shutdown_requested() {
//!     println!("Shutdown requested, cleaning up...");
//!     return;
//! }
//!
//! // Get the flag to pass to worker threads
//! let shutdown_flag = handler.get_flag();
//! // Pass shutdown_flag to DuplicateFinder, Walker, etc.
//! ```
//!
//! # Exit Codes
//!
//! When a signal is received:
//! - The shutdown flag is set to `true`
//! - A message "Interrupted. Cleaning up..." is printed to stderr
//! - The application should exit with code 130 (128 + SIGINT)

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Exit code for SIGINT (Ctrl+C) interruption.
/// This follows Unix convention: 128 + signal number (SIGINT = 2).
pub const EXIT_CODE_INTERRUPTED: i32 = 130;

/// Centralized shutdown handler for graceful application termination.
///
/// This struct wraps an `AtomicBool` flag that is set when a Ctrl+C signal
/// is received. The flag can be shared with worker threads to enable
/// coordinated shutdown.
///
/// # Thread Safety
///
/// `ShutdownHandler` is `Send` and `Sync`, and the underlying flag uses
/// atomic operations for thread-safe access.
///
/// # Example
///
/// ```rust,no_run
/// use rustdupe::signal::ShutdownHandler;
///
/// let handler = ShutdownHandler::new();
///
/// // In main thread
/// if handler.is_shutdown_requested() {
///     // Clean up and exit
/// }
///
/// // In worker thread (pass the flag)
/// let flag = handler.get_flag();
/// // Worker checks: flag.load(Ordering::SeqCst)
/// ```
#[derive(Debug, Clone)]
pub struct ShutdownHandler {
    /// The shared atomic flag indicating shutdown was requested.
    flag: Arc<AtomicBool>,
}

impl ShutdownHandler {
    /// Create a new shutdown handler with the flag initially set to `false`.
    ///
    /// # Returns
    ///
    /// A new `ShutdownHandler` with no shutdown requested.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if shutdown has been requested.
    ///
    /// # Returns
    ///
    /// `true` if Ctrl+C was pressed or `request_shutdown()` was called.
    #[must_use]
    pub fn is_shutdown_requested(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Manually request a shutdown.
    ///
    /// This sets the flag to `true`, which will be observed by any code
    /// checking `is_shutdown_requested()` or using `get_flag()`.
    pub fn request_shutdown(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Get a clone of the shutdown flag for passing to worker threads.
    ///
    /// This is the primary way to share the shutdown signal with other
    /// components like `DuplicateFinder`, `Walker`, and `Hasher`.
    ///
    /// # Returns
    ///
    /// An `Arc<AtomicBool>` that can be passed to worker threads.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rustdupe::signal::ShutdownHandler;
    /// use rustdupe::duplicates::FinderConfig;
    ///
    /// let handler = ShutdownHandler::new();
    /// let config = FinderConfig::default()
    ///     .with_shutdown_flag(handler.get_flag());
    /// ```
    #[must_use]
    pub fn get_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }

    /// Reset the shutdown flag to `false`.
    ///
    /// This is primarily useful for testing scenarios where you want to
    /// reuse a handler.
    pub fn reset(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

impl Default for ShutdownHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for signal handler installation.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    /// Failed to install the Ctrl+C handler.
    #[error("Failed to install signal handler: {0}")]
    InstallFailed(#[from] ctrlc::Error),
}

/// Install a Ctrl+C handler that sets the shutdown flag on interrupt.
///
/// This function should be called once, early in the application startup,
/// before any long-running operations begin.
///
/// When Ctrl+C is pressed:
/// 1. The shutdown flag is set to `true`
/// 2. A message "Interrupted. Cleaning up..." is printed to stderr
/// 3. Any code checking `is_shutdown_requested()` will see `true`
///
/// # Returns
///
/// A `ShutdownHandler` that can be used to check shutdown status and
/// get the flag for worker threads.
///
/// # Errors
///
/// Returns `SignalError::InstallFailed` if the ctrlc handler cannot be installed.
/// This can happen if a handler is already installed.
///
/// # Example
///
/// ```rust,no_run
/// use rustdupe::signal::{install_handler, EXIT_CODE_INTERRUPTED};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let handler = install_handler()?;
///
///     // Pass the flag to workers
///     let shutdown_flag = handler.get_flag();
///
///     // Do work...
///
///     // Check for shutdown
///     if handler.is_shutdown_requested() {
///         std::process::exit(EXIT_CODE_INTERRUPTED);
///     }
///
///     Ok(())
/// }
/// ```
pub fn install_handler() -> Result<ShutdownHandler, SignalError> {
    let handler = ShutdownHandler::new();
    let flag = handler.get_flag();

    ctrlc::set_handler(move || {
        // Set the shutdown flag
        flag.store(true, Ordering::SeqCst);

        // Print message to stderr (stderr is line-buffered, so flush explicitly)
        let _ = writeln!(std::io::stderr(), "\nInterrupted. Cleaning up...");
        let _ = std::io::stderr().flush();

        log::info!("Shutdown signal received");
    })?;

    log::debug!("Ctrl+C signal handler installed");

    Ok(handler)
}

/// Create a handler without installing any signal hooks.
///
/// This is useful for testing or when you want to manage the shutdown
/// flag manually without actual signal handling.
///
/// # Returns
///
/// A `ShutdownHandler` with the flag set to `false`.
/// # Example
///
/// ```
/// use rustdupe::signal::create_handler;
/// let handler = create_handler();
/// ```
#[must_use]
pub fn create_handler() -> ShutdownHandler {
    ShutdownHandler::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_handler_new() {
        let handler = ShutdownHandler::new();
        assert!(!handler.is_shutdown_requested());
    }

    #[test]
    fn test_shutdown_handler_default() {
        let handler = ShutdownHandler::default();
        assert!(!handler.is_shutdown_requested());
    }

    #[test]
    fn test_request_shutdown() {
        let handler = ShutdownHandler::new();
        assert!(!handler.is_shutdown_requested());

        handler.request_shutdown();
        assert!(handler.is_shutdown_requested());
    }

    #[test]
    fn test_reset() {
        let handler = ShutdownHandler::new();
        handler.request_shutdown();
        assert!(handler.is_shutdown_requested());

        handler.reset();
        assert!(!handler.is_shutdown_requested());
    }

    #[test]
    fn test_get_flag_shares_state() {
        let handler = ShutdownHandler::new();
        let flag = handler.get_flag();

        assert!(!flag.load(Ordering::SeqCst));

        handler.request_shutdown();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_flag_modification_reflects_in_handler() {
        let handler = ShutdownHandler::new();
        let flag = handler.get_flag();

        flag.store(true, Ordering::SeqCst);
        assert!(handler.is_shutdown_requested());
    }

    #[test]
    fn test_clone_shares_flag() {
        let handler = ShutdownHandler::new();
        let cloned = handler.clone();

        handler.request_shutdown();
        assert!(cloned.is_shutdown_requested());
    }

    #[test]
    fn test_create_handler() {
        let handler = create_handler();
        assert!(!handler.is_shutdown_requested());
    }

    #[test]
    fn test_exit_code_interrupted() {
        assert_eq!(EXIT_CODE_INTERRUPTED, 130);
    }

    #[test]
    fn test_signal_error_display() {
        // We can't easily create a ctrlc::Error, but we can test the Display impl
        // by checking that SignalError implements Display
        fn assert_display<T: std::fmt::Display>() {}
        assert_display::<SignalError>();
    }

    #[test]
    fn test_signal_error_debug() {
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<SignalError>();
    }

    #[test]
    fn test_shutdown_handler_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ShutdownHandler>();
    }
}
