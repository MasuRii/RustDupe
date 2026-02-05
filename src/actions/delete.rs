//! Safe file deletion using trash crate.
//!
//! # Overview
//!
//! This module provides safe file deletion functionality:
//! - Move to system trash (default, recoverable)
//! - Permanent deletion (with explicit flag)
//! - Batch operations with progress reporting
//! - TOCTOU verification before deletion
//!
//! # Safety
//!
//! All deletion operations verify file existence before attempting deletion.
//! At least one copy is always preserved when deleting from duplicate groups.
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::actions::delete::{delete_to_trash, DeleteConfig};
//! use std::path::PathBuf;
//!
//! // Single file deletion to trash
//! let path = PathBuf::from("/path/to/duplicate.txt");
//! match delete_to_trash(&path) {
//!     Ok(result) => println!("Deleted: {}", result.path.display()),
//!     Err(e) => eprintln!("Failed: {}", e),
//! }
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use thiserror::Error;

/// Error type for deletion operations.
#[derive(Debug, Error)]
pub enum DeleteError {
    /// File was not found (may have been deleted or moved).
    #[error("file not found: {0}")]
    NotFound(PathBuf),

    /// Permission denied when attempting to delete.
    #[error("permission denied: {0} - try running with elevated privileges")]
    PermissionDenied(PathBuf),

    /// File was modified since scan (TOCTOU protection).
    #[error("file modified since scan: {0}")]
    Modified(PathBuf),

    /// Trash operation failed.
    #[error("trash operation failed for {path}: {message}")]
    TrashFailed { path: PathBuf, message: String },

    /// Permanent delete operation failed.
    #[error("permanent delete failed for {path}: {message}")]
    PermanentDeleteFailed { path: PathBuf, message: String },

    /// Attempted to delete all copies (at least one must be preserved).
    #[error("cannot delete all copies - at least one file must be preserved")]
    AllCopiesWouldBeDeleted,

    /// General I/O error.
    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl DeleteError {
    /// Get the path associated with this error (if any).
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::NotFound(p)
            | Self::PermissionDenied(p)
            | Self::Modified(p)
            | Self::TrashFailed { path: p, .. }
            | Self::PermanentDeleteFailed { path: p, .. }
            | Self::Io { path: p, .. } => Some(p),
            Self::AllCopiesWouldBeDeleted => None,
        }
    }
}

/// Result of a successful deletion operation.
#[derive(Debug, Clone)]
pub struct DeleteResult {
    /// Path that was deleted.
    pub path: PathBuf,
    /// Size of the deleted file in bytes.
    pub size: u64,
    /// Whether deletion was permanent (true) or to trash (false).
    pub permanent: bool,
}

impl DeleteResult {
    /// Create a new delete result.
    #[must_use]
    pub fn new(path: PathBuf, size: u64, permanent: bool) -> Self {
        Self {
            path,
            size,
            permanent,
        }
    }
}

/// Results of a batch deletion operation.
#[derive(Debug, Clone, Default)]
pub struct BatchDeleteResult {
    /// Successfully deleted files.
    pub successes: Vec<DeleteResult>,
    /// Failed deletions with their errors.
    pub failures: Vec<(PathBuf, String)>,
    /// Total bytes freed.
    pub bytes_freed: u64,
}

impl BatchDeleteResult {
    /// Number of successful deletions.
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.successes.len()
    }

    /// Number of failed deletions.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.failures.len()
    }

    /// Total number of attempted deletions.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.successes.len() + self.failures.len()
    }

    /// Check if all deletions succeeded.
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        self.failures.is_empty()
    }

    /// Human-readable summary of the operation.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.all_succeeded() {
            format!(
                "Deleted {} file(s), freed {} bytes",
                self.success_count(),
                self.bytes_freed
            )
        } else {
            format!(
                "Deleted {} file(s), {} failed, freed {} bytes",
                self.success_count(),
                self.failure_count(),
                self.bytes_freed
            )
        }
    }
}

/// Configuration for deletion operations.
#[derive(Debug, Clone)]
pub struct DeleteConfig {
    /// Use permanent deletion instead of trash.
    pub permanent: bool,
    /// Verify file modification time before deletion (TOCTOU protection).
    pub verify_mtime: bool,
    /// Continue on error (process remaining files even if some fail).
    pub continue_on_error: bool,
}

impl Default for DeleteConfig {
    fn default() -> Self {
        Self {
            permanent: false,
            verify_mtime: true,
            continue_on_error: true,
        }
    }
}

impl DeleteConfig {
    /// Create config for trash deletion.
    #[must_use]
    pub fn trash() -> Self {
        Self::default()
    }

    /// Create config for permanent deletion.
    #[must_use]
    pub fn permanent() -> Self {
        Self {
            permanent: true,
            ..Self::default()
        }
    }

    /// Enable/disable TOCTOU verification.
    #[must_use]
    pub fn with_verify_mtime(mut self, verify: bool) -> Self {
        self.verify_mtime = verify;
        self
    }

    /// Enable/disable continue on error.
    #[must_use]
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }
}

/// Callback trait for deletion progress reporting.
pub trait DeleteProgressCallback: Send + Sync {
    /// Called before each file deletion.
    fn on_before_delete(&self, path: &Path, index: usize, total: usize);

    /// Called after successful deletion.
    fn on_delete_success(&self, path: &Path, size: u64);

    /// Called after failed deletion.
    fn on_delete_failure(&self, path: &Path, error: &str);

    /// Called when batch operation completes.
    fn on_complete(&self, result: &BatchDeleteResult);
}

/// File metadata snapshot for TOCTOU verification.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    /// Path to the file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub mtime: Option<SystemTime>,
}

impl FileSnapshot {
    /// Create a snapshot of a file's current state.
    ///
    /// # Errors
    ///
    /// Returns error if file doesn't exist or can't be accessed.
    pub fn capture(path: &Path) -> Result<Self, DeleteError> {
        let metadata = fs::metadata(path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => DeleteError::NotFound(path.to_path_buf()),
            io::ErrorKind::PermissionDenied => DeleteError::PermissionDenied(path.to_path_buf()),
            _ => DeleteError::Io {
                path: path.to_path_buf(),
                source: e,
            },
        })?;

        Ok(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            mtime: metadata.modified().ok(),
        })
    }

    /// Verify that the file still matches this snapshot.
    ///
    /// # Errors
    ///
    /// Returns error if file was modified, deleted, or can't be accessed.
    pub fn verify(&self) -> Result<(), DeleteError> {
        let current = Self::capture(&self.path)?;

        // Check if mtime changed (if we have both mtimes)
        if let (Some(orig), Some(curr)) = (self.mtime, current.mtime) {
            if orig != curr {
                log::warn!(
                    "File modified since scan: {} (mtime changed)",
                    self.path.display()
                );
                return Err(DeleteError::Modified(self.path.clone()));
            }
        }

        // Check if size changed
        if self.size != current.size {
            log::warn!(
                "File modified since scan: {} (size changed from {} to {})",
                self.path.display(),
                self.size,
                current.size
            );
            return Err(DeleteError::Modified(self.path.clone()));
        }

        Ok(())
    }
}

/// Delete a single file to the system trash.
///
/// This is the safest deletion method - files can be recovered from trash.
///
/// # Arguments
///
/// * `path` - Path to the file to delete
///
/// # Returns
///
/// A `DeleteResult` on success, or `DeleteError` on failure.
///
/// # Errors
///
/// - `NotFound` if the file doesn't exist
/// - `PermissionDenied` if deletion is not allowed
/// - `TrashFailed` if the trash operation fails
///
/// # Example
///
/// ```no_run
/// use rustdupe::actions::delete::delete_to_trash;
/// use std::path::PathBuf;
///
/// let path = PathBuf::from("/path/to/file.txt");
/// match delete_to_trash(&path) {
///     Ok(result) => println!("Moved to trash: {}", result.path.display()),
///     Err(e) => eprintln!("Failed: {}", e),
/// }
/// ```
pub fn delete_to_trash(path: &Path) -> Result<DeleteResult, DeleteError> {
    // Get file size before deletion
    let metadata = fs::metadata(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => DeleteError::NotFound(path.to_path_buf()),
        io::ErrorKind::PermissionDenied => DeleteError::PermissionDenied(path.to_path_buf()),
        _ => DeleteError::Io {
            path: path.to_path_buf(),
            source: e,
        },
    })?;

    let size = metadata.len();

    // Move to trash
    trash::delete(path).map_err(|e| {
        log::error!("Trash operation failed for {}: {}", path.display(), e);
        DeleteError::TrashFailed {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;

    log::info!("Moved to trash: {} ({} bytes)", path.display(), size);

    Ok(DeleteResult::new(path.to_path_buf(), size, false))
}

/// Permanently delete a single file.
///
/// **WARNING**: This operation cannot be undone. The file will be permanently removed.
///
/// # Arguments
///
/// * `path` - Path to the file to delete
///
/// # Returns
///
/// A `DeleteResult` on success, or `DeleteError` on failure.
///
/// # Errors
///
/// - `NotFound` if the file doesn't exist
/// - `PermissionDenied` if deletion is not allowed
/// - `PermanentDeleteFailed` if the delete operation fails
///
/// # Example
///
/// ```no_run
/// use rustdupe::actions::delete::permanent_delete;
/// use std::path::PathBuf;
///
/// let path = PathBuf::from("/path/to/file.txt");
/// match permanent_delete(&path) {
///     Ok(result) => println!("Permanently deleted: {}", result.path.display()),
///     Err(e) => eprintln!("Failed: {}", e),
/// }
/// ```
pub fn permanent_delete(path: &Path) -> Result<DeleteResult, DeleteError> {
    // Get file size before deletion
    let metadata = fs::metadata(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => DeleteError::NotFound(path.to_path_buf()),
        io::ErrorKind::PermissionDenied => DeleteError::PermissionDenied(path.to_path_buf()),
        _ => DeleteError::Io {
            path: path.to_path_buf(),
            source: e,
        },
    })?;

    let size = metadata.len();

    // Permanently delete
    fs::remove_file(path).map_err(|e| {
        log::error!("Permanent delete failed for {}: {}", path.display(), e);
        DeleteError::PermanentDeleteFailed {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;

    log::info!("Permanently deleted: {} ({} bytes)", path.display(), size);

    Ok(DeleteResult::new(path.to_path_buf(), size, true))
}

/// Delete a single file with TOCTOU verification.
///
/// Verifies the file hasn't changed since it was scanned before deleting.
///
/// # Arguments
///
/// * `path` - Path to the file to delete
/// * `expected_mtime` - Expected modification time (from scan)
/// * `config` - Deletion configuration
///
/// # Errors
///
/// - `Modified` if the file was changed since scan
/// - Other errors from `delete_to_trash` or `permanent_delete`
pub fn delete_verified(
    path: &Path,
    expected_mtime: Option<SystemTime>,
    config: &DeleteConfig,
) -> Result<DeleteResult, DeleteError> {
    // TOCTOU verification
    if config.verify_mtime {
        // Get current metadata
        let current = FileSnapshot::capture(path)?;

        // Compare mtime if available
        if let (Some(expected), Some(actual)) = (expected_mtime, current.mtime) {
            if expected != actual {
                return Err(DeleteError::Modified(path.to_path_buf()));
            }
        }
    }

    // Perform deletion
    if config.permanent {
        permanent_delete(path)
    } else {
        delete_to_trash(path)
    }
}

/// Delete multiple files in batch.
///
/// Processes all files, continuing on error if configured to do so.
/// At least one file in each duplicate group should be preserved before calling this.
///
/// # Arguments
///
/// * `paths` - Slice of paths to delete
/// * `config` - Deletion configuration
/// * `callback` - Optional progress callback
///
/// # Returns
///
/// A `BatchDeleteResult` with success/failure information.
///
/// # Example
///
/// ```no_run
/// use rustdupe::actions::delete::{delete_batch, DeleteConfig, DeleteProgressCallback, BatchDeleteResult};
/// use std::path::{Path, PathBuf};
///
/// // Define a simple no-op callback (required for type inference)
/// struct NoCallback;
/// impl DeleteProgressCallback for NoCallback {
///     fn on_before_delete(&self, _: &Path, _: usize, _: usize) {}
///     fn on_delete_success(&self, _: &Path, _: u64) {}
///     fn on_delete_failure(&self, _: &Path, _: &str) {}
///     fn on_complete(&self, _: &BatchDeleteResult) {}
/// }
///
/// let paths = vec![
///     PathBuf::from("/dup1.txt"),
///     PathBuf::from("/dup2.txt"),
/// ];
///
/// let result = delete_batch::<NoCallback>(&paths, &DeleteConfig::default(), None);
/// println!("{}", result.summary());
/// ```
pub fn delete_batch<C: DeleteProgressCallback>(
    paths: &[PathBuf],
    config: &DeleteConfig,
    callback: Option<&C>,
) -> BatchDeleteResult {
    let mut result = BatchDeleteResult::default();
    let total = paths.len();

    for (index, path) in paths.iter().enumerate() {
        // Progress callback
        if let Some(cb) = callback {
            cb.on_before_delete(path, index, total);
        }

        // Attempt deletion
        let delete_result = if config.permanent {
            permanent_delete(path)
        } else {
            delete_to_trash(path)
        };

        match delete_result {
            Ok(del) => {
                result.bytes_freed += del.size;
                if let Some(cb) = callback {
                    cb.on_delete_success(path, del.size);
                }
                result.successes.push(del);
            }
            Err(e) => {
                let error_msg = e.to_string();
                log::warn!("Failed to delete {}: {}", path.display(), error_msg);

                if let Some(cb) = callback {
                    cb.on_delete_failure(path, &error_msg);
                }

                result.failures.push((path.clone(), error_msg));

                if !config.continue_on_error {
                    log::info!("Stopping batch deletion due to error (continue_on_error=false)");
                    break;
                }
            }
        }
    }

    // Completion callback
    if let Some(cb) = callback {
        cb.on_complete(&result);
    }

    log::info!("{}", result.summary());

    result
}

/// Validate that a selection doesn't delete all copies.
///
/// At least one copy of each duplicate group must be preserved.
///
/// # Arguments
///
/// * `selected_paths` - Paths selected for deletion
/// * `group_paths` - All paths in the duplicate group
///
/// # Errors
///
/// Returns `AllCopiesWouldBeDeleted` if all copies would be deleted.
///
/// # Example
///
/// ```
/// use rustdupe::actions::delete::validate_preserves_copy;
/// use std::path::PathBuf;
///
/// let group = vec![
///     PathBuf::from("/original.txt"),
///     PathBuf::from("/copy1.txt"),
///     PathBuf::from("/copy2.txt"),
/// ];
///
/// // This is OK - one copy preserved
/// let selected = vec![
///     PathBuf::from("/copy1.txt"),
///     PathBuf::from("/copy2.txt"),
/// ];
/// assert!(validate_preserves_copy(&selected, &group).is_ok());
///
/// // This would delete all copies - error
/// let all_selected = group.clone();
/// assert!(validate_preserves_copy(&all_selected, &group).is_err());
/// ```
pub fn validate_preserves_copy(
    selected_paths: &[PathBuf],
    group_paths: &[PathBuf],
) -> Result<(), DeleteError> {
    use std::collections::HashSet;

    let selected_set: HashSet<&PathBuf> = selected_paths.iter().collect();
    let preserved_count = group_paths
        .iter()
        .filter(|p| !selected_set.contains(p))
        .count();

    if preserved_count == 0 {
        log::error!(
            "Attempted to delete all {} copies of a duplicate group",
            group_paths.len()
        );
        Err(DeleteError::AllCopiesWouldBeDeleted)
    } else {
        log::debug!(
            "Deletion validated: {} files selected, {} preserved",
            selected_paths.len(),
            preserved_count
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_temp_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = fs::File::create(&path).expect("Failed to create temp file");
        file.write_all(content).expect("Failed to write content");
        path
    }

    // ==================== DeleteError Tests ====================

    #[test]
    fn test_delete_error_path() {
        let path = PathBuf::from("/test/path");

        assert_eq!(
            DeleteError::NotFound(path.clone()).path(),
            Some(path.as_path())
        );
        assert_eq!(
            DeleteError::PermissionDenied(path.clone()).path(),
            Some(path.as_path())
        );
        assert_eq!(
            DeleteError::Modified(path.clone()).path(),
            Some(path.as_path())
        );
        assert_eq!(DeleteError::AllCopiesWouldBeDeleted.path(), None);
    }

    #[test]
    fn test_delete_error_display() {
        let path = PathBuf::from("/test/file.txt");

        let err = DeleteError::NotFound(path.clone());
        assert!(err.to_string().contains("not found"));

        let err = DeleteError::Modified(path.clone());
        assert!(err.to_string().contains("modified"));

        let err = DeleteError::AllCopiesWouldBeDeleted;
        assert!(err.to_string().contains("at least one"));
    }

    // ==================== DeleteResult Tests ====================

    #[test]
    fn test_delete_result_new() {
        let result = DeleteResult::new(PathBuf::from("/test.txt"), 1024, false);

        assert_eq!(result.path, PathBuf::from("/test.txt"));
        assert_eq!(result.size, 1024);
        assert!(!result.permanent);
    }

    // ==================== BatchDeleteResult Tests ====================

    #[test]
    fn test_batch_delete_result_default() {
        let result = BatchDeleteResult::default();

        assert_eq!(result.success_count(), 0);
        assert_eq!(result.failure_count(), 0);
        assert_eq!(result.total_count(), 0);
        assert!(result.all_succeeded());
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn test_batch_delete_result_with_successes() {
        let mut result = BatchDeleteResult::default();
        result
            .successes
            .push(DeleteResult::new(PathBuf::from("/a.txt"), 1000, false));
        result
            .successes
            .push(DeleteResult::new(PathBuf::from("/b.txt"), 2000, false));
        result.bytes_freed = 3000;

        assert_eq!(result.success_count(), 2);
        assert_eq!(result.failure_count(), 0);
        assert!(result.all_succeeded());
        assert!(result.summary().contains("2 file(s)"));
        assert!(result.summary().contains("3000"));
    }

    #[test]
    fn test_batch_delete_result_with_failures() {
        let mut result = BatchDeleteResult::default();
        result
            .successes
            .push(DeleteResult::new(PathBuf::from("/a.txt"), 1000, false));
        result
            .failures
            .push((PathBuf::from("/b.txt"), "permission denied".to_string()));

        assert_eq!(result.success_count(), 1);
        assert_eq!(result.failure_count(), 1);
        assert_eq!(result.total_count(), 2);
        assert!(!result.all_succeeded());
        assert!(result.summary().contains("1 failed"));
    }

    // ==================== DeleteConfig Tests ====================

    #[test]
    fn test_delete_config_default() {
        let config = DeleteConfig::default();

        assert!(!config.permanent);
        assert!(config.verify_mtime);
        assert!(config.continue_on_error);
    }

    #[test]
    fn test_delete_config_trash() {
        let config = DeleteConfig::trash();
        assert!(!config.permanent);
    }

    #[test]
    fn test_delete_config_permanent() {
        let config = DeleteConfig::permanent();
        assert!(config.permanent);
    }

    #[test]
    fn test_delete_config_builders() {
        let config = DeleteConfig::default()
            .with_verify_mtime(false)
            .with_continue_on_error(false);

        assert!(!config.verify_mtime);
        assert!(!config.continue_on_error);
    }

    // ==================== FileSnapshot Tests ====================

    #[test]
    fn test_file_snapshot_capture() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_temp_file(&dir, "test.txt", b"hello");

        let snapshot = FileSnapshot::capture(&path).expect("Failed to capture snapshot");

        assert_eq!(snapshot.path, path);
        assert_eq!(snapshot.size, 5);
        assert!(snapshot.mtime.is_some());
    }

    #[test]
    fn test_file_snapshot_capture_not_found() {
        let path = PathBuf::from("/nonexistent/file.txt");
        let result = FileSnapshot::capture(&path);

        assert!(matches!(result, Err(DeleteError::NotFound(_))));
    }

    #[test]
    fn test_file_snapshot_verify_unchanged() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_temp_file(&dir, "test.txt", b"hello");

        let snapshot = FileSnapshot::capture(&path).expect("Failed to capture snapshot");
        let result = snapshot.verify();

        assert!(result.is_ok());
    }

    #[test]
    fn test_file_snapshot_verify_deleted() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_temp_file(&dir, "test.txt", b"hello");

        let snapshot = FileSnapshot::capture(&path).expect("Failed to capture snapshot");

        // Delete the file
        fs::remove_file(&path).expect("Failed to delete file");

        let result = snapshot.verify();
        assert!(matches!(result, Err(DeleteError::NotFound(_))));
    }

    // ==================== validate_preserves_copy Tests ====================

    #[test]
    fn test_validate_preserves_copy_one_preserved() {
        let group = vec![
            PathBuf::from("/a.txt"),
            PathBuf::from("/b.txt"),
            PathBuf::from("/c.txt"),
        ];
        let selected = vec![PathBuf::from("/b.txt"), PathBuf::from("/c.txt")];

        let result = validate_preserves_copy(&selected, &group);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_preserves_copy_all_preserved() {
        let group = vec![PathBuf::from("/a.txt"), PathBuf::from("/b.txt")];
        let selected = vec![];

        let result = validate_preserves_copy(&selected, &group);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_preserves_copy_none_preserved() {
        let group = vec![PathBuf::from("/a.txt"), PathBuf::from("/b.txt")];
        let selected = vec![PathBuf::from("/a.txt"), PathBuf::from("/b.txt")];

        let result = validate_preserves_copy(&selected, &group);
        assert!(matches!(result, Err(DeleteError::AllCopiesWouldBeDeleted)));
    }

    #[test]
    fn test_validate_preserves_copy_single_file_group() {
        let group = vec![PathBuf::from("/a.txt")];
        let selected = vec![PathBuf::from("/a.txt")];

        let result = validate_preserves_copy(&selected, &group);
        assert!(matches!(result, Err(DeleteError::AllCopiesWouldBeDeleted)));
    }

    #[test]
    fn test_validate_preserves_copy_empty_selection() {
        let group = vec![PathBuf::from("/a.txt")];
        let selected: Vec<PathBuf> = vec![];

        let result = validate_preserves_copy(&selected, &group);
        assert!(result.is_ok());
    }

    // ==================== permanent_delete Tests ====================

    #[test]
    fn test_permanent_delete_success() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_temp_file(&dir, "delete_me.txt", b"test content");

        assert!(path.exists());

        let result = permanent_delete(&path).expect("Failed to delete");

        assert!(!path.exists());
        assert_eq!(result.size, 12); // "test content"
        assert!(result.permanent);
    }

    #[test]
    fn test_permanent_delete_not_found() {
        let path = PathBuf::from("/nonexistent/file.txt");
        let result = permanent_delete(&path);

        assert!(matches!(result, Err(DeleteError::NotFound(_))));
    }

    // ==================== delete_to_trash Tests ====================

    #[test]
    fn test_delete_to_trash_not_found() {
        let path = PathBuf::from("/nonexistent/file.txt");
        let result = delete_to_trash(&path);

        assert!(matches!(result, Err(DeleteError::NotFound(_))));
    }

    // Note: Actual trash tests are platform-dependent and may not work in all environments.
    // The trash crate handles the platform-specific implementation.

    // ==================== delete_batch Tests ====================

    #[test]
    fn test_delete_batch_empty() {
        let paths: Vec<PathBuf> = vec![];
        let config = DeleteConfig::permanent();

        let result = delete_batch::<NoOpCallback>(&paths, &config, None);

        assert_eq!(result.success_count(), 0);
        assert_eq!(result.failure_count(), 0);
        assert!(result.all_succeeded());
    }

    #[test]
    fn test_delete_batch_with_failures() {
        let paths = vec![
            PathBuf::from("/nonexistent1.txt"),
            PathBuf::from("/nonexistent2.txt"),
        ];
        let config = DeleteConfig::permanent();

        let result = delete_batch::<NoOpCallback>(&paths, &config, None);

        assert_eq!(result.success_count(), 0);
        assert_eq!(result.failure_count(), 2);
        assert!(!result.all_succeeded());
    }

    #[test]
    fn test_delete_batch_mixed() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let existing = create_temp_file(&dir, "exists.txt", b"content");
        let nonexistent = PathBuf::from("/nonexistent.txt");

        let paths = vec![existing.clone(), nonexistent];
        let config = DeleteConfig::permanent();

        let result = delete_batch::<NoOpCallback>(&paths, &config, None);

        assert_eq!(result.success_count(), 1);
        assert_eq!(result.failure_count(), 1);
        assert!(!existing.exists());
    }

    #[test]
    fn test_delete_batch_stop_on_error() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let nonexistent = PathBuf::from("/nonexistent.txt");
        let existing = create_temp_file(&dir, "exists.txt", b"content");

        // Put nonexistent first to trigger error
        let paths = vec![nonexistent, existing.clone()];
        let config = DeleteConfig::permanent().with_continue_on_error(false);

        let result = delete_batch::<NoOpCallback>(&paths, &config, None);

        // Should stop after first error
        assert_eq!(result.total_count(), 1);
        assert_eq!(result.failure_count(), 1);
        // Second file should still exist
        assert!(existing.exists());
    }

    #[test]
    fn test_delete_batch_with_callback() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_temp_file(&dir, "test.txt", b"content");

        let paths = vec![path.clone()];
        let config = DeleteConfig::permanent();
        let callback = TestCallback::new();

        let result = delete_batch(&paths, &config, Some(&callback));

        assert_eq!(result.success_count(), 1);
        assert!(callback.before_count() >= 1);
        assert!(callback.success_count() >= 1);
        assert!(callback.complete_called());
    }

    // ==================== Test Helpers ====================

    /// No-op callback for tests that don't need progress reporting.
    struct NoOpCallback;

    impl DeleteProgressCallback for NoOpCallback {
        fn on_before_delete(&self, _path: &Path, _index: usize, _total: usize) {}
        fn on_delete_success(&self, _path: &Path, _size: u64) {}
        fn on_delete_failure(&self, _path: &Path, _error: &str) {}
        fn on_complete(&self, _result: &BatchDeleteResult) {}
    }

    /// Test callback that tracks calls.
    struct TestCallback {
        before: std::sync::atomic::AtomicUsize,
        success: std::sync::atomic::AtomicUsize,
        failure: std::sync::atomic::AtomicUsize,
        complete: std::sync::atomic::AtomicBool,
    }

    impl TestCallback {
        fn new() -> Self {
            Self {
                before: std::sync::atomic::AtomicUsize::new(0),
                success: std::sync::atomic::AtomicUsize::new(0),
                failure: std::sync::atomic::AtomicUsize::new(0),
                complete: std::sync::atomic::AtomicBool::new(false),
            }
        }

        fn before_count(&self) -> usize {
            self.before.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn success_count(&self) -> usize {
            self.success.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn complete_called(&self) -> bool {
            self.complete.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl DeleteProgressCallback for TestCallback {
        fn on_before_delete(&self, _path: &Path, _index: usize, _total: usize) {
            self.before
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn on_delete_success(&self, _path: &Path, _size: u64) {
            self.success
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn on_delete_failure(&self, _path: &Path, _error: &str) {
            self.failure
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn on_complete(&self, _result: &BatchDeleteResult) {
            self.complete
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
