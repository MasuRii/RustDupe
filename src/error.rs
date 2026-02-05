//! Structured error handling and exit codes.

use serde::Serialize;

/// Exit codes for the RustDupe application.
///
/// Following the recommendations in Task 2.4.3:
/// - 0: Success (completed normally, duplicates found)
/// - 1: General error (unexpected failure)
/// - 2: No duplicates found (completed normally, no duplicates)
/// - 3: Partial success (completed with some non-fatal scan errors)
/// - 130: Interrupted by user (Ctrl+C)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ExitCode {
    /// Success: Scan completed and duplicates were found.
    Success = 0,
    /// General error: An unexpected error occurred.
    GeneralError = 1,
    /// No duplicates: Scan completed but no duplicates were found.
    NoDuplicates = 2,
    /// Partial success: Scan completed but encountered some non-fatal errors.
    PartialSuccess = 3,
    /// Interrupted: Scan was interrupted by user (Ctrl+C).
    Interrupted = 130,
}

impl ExitCode {
    /// Get the numeric exit code.
    #[must_use]
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// Get the machine-readable code prefix.
    #[must_use]
    pub fn code_prefix(self) -> &'static str {
        match self {
            Self::Success => "RD000",
            Self::GeneralError => "RD001",
            Self::NoDuplicates => "RD002",
            Self::PartialSuccess => "RD003",
            Self::Interrupted => "RD130",
        }
    }
}

/// Structured error information for JSON output.
#[derive(Debug, Serialize)]
pub struct StructuredError {
    /// The error code (e.g., "RD001")
    pub code: String,
    /// The exit code number
    pub exit_code: i32,
    /// Human-readable error message
    pub message: String,
    /// Whether the operation was interrupted
    pub interrupted: bool,
}

impl StructuredError {
    /// Create a new structured error from an anyhow error and an exit code.
    #[must_use]
    pub fn new(err: &anyhow::Error, exit_code: ExitCode) -> Self {
        Self {
            code: exit_code.code_prefix().to_string(),
            exit_code: exit_code.as_i32(),
            message: err.to_string(),
            interrupted: exit_code == ExitCode::Interrupted,
        }
    }
}
