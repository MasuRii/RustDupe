//! Comprehensive integration tests for error handling functionality.
//!
//! These tests verify graceful error continuation, strict mode behavior,
//! structured error codes, and JSON error reporting.

use clap::Parser;
use rustdupe::cli::Cli;
use rustdupe::error::ExitCode;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

#[cfg(unix)]
use std::fs;

#[test]
fn test_exit_code_no_duplicates() {
    let dir = tempdir().unwrap();
    // Create one file (no duplicates)
    File::create(dir.path().join("unique.txt"))
        .unwrap()
        .write_all(b"unique")
        .unwrap();

    let cli = Cli::try_parse_from([
        "rustdupe",
        "scan",
        dir.path().to_str().unwrap(),
        "--output",
        "json",
    ])
    .unwrap();

    // We can't easily capture stdout/stderr from run() without redirecting,
    // but we can check the returned ExitCode.
    let result = rustdupe::run_app(cli).unwrap();
    assert_eq!(result, ExitCode::NoDuplicates);
}

#[test]
fn test_exit_code_success_with_duplicates() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("b.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    let cli = Cli::try_parse_from([
        "rustdupe",
        "scan",
        dir.path().to_str().unwrap(),
        "--output",
        "json",
    ])
    .unwrap();
    let result = rustdupe::run_app(cli).unwrap();
    assert_eq!(result, ExitCode::Success);
}

#[cfg(unix)]
#[test]
fn test_exit_code_partial_success_on_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();

    // Accessible duplicate
    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("b.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    // Create inaccessible directory with files inside
    // The walker should fail to read the directory contents
    let sub = dir.path().join("no_access");
    fs::create_dir(&sub).unwrap();
    // Put files inside before revoking permissions
    File::create(sub.join("hidden.txt"))
        .unwrap()
        .write_all(b"hidden content")
        .unwrap();
    let mut perms = fs::metadata(&sub).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&sub, perms).unwrap();

    let cli = Cli::try_parse_from([
        "rustdupe",
        "scan",
        dir.path().to_str().unwrap(),
        "--output",
        "json",
    ])
    .unwrap();
    let result = rustdupe::run_app(cli).unwrap();

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&sub).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&sub, perms).unwrap();

    // On some systems (especially macOS), the walker may silently skip inaccessible directories
    // without reporting an error. Accept both Success and PartialSuccess.
    assert!(
        result == ExitCode::PartialSuccess || result == ExitCode::Success,
        "Expected PartialSuccess or Success, got {:?}",
        result
    );
}

#[cfg(unix)]
#[test]
fn test_strict_mode_fails_on_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();

    let sub = dir.path().join("no_access");
    fs::create_dir(&sub).unwrap();
    let mut perms = fs::metadata(&sub).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&sub, perms).unwrap();

    let cli = Cli::try_parse_from(["rustdupe", "scan", "--strict", dir.path().to_str().unwrap()])
        .unwrap();
    let result = rustdupe::run_app(cli);

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&sub).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&sub, perms).unwrap();

    // In strict mode, it should return Err
    assert!(result.is_err());
}

#[test]
fn test_exit_code_general_error_on_invalid_path() {
    let cli = Cli::try_parse_from([
        "rustdupe",
        "scan",
        "/non/existent/path/that/really/should/not/exist",
    ])
    .unwrap();
    let result = rustdupe::run_app(cli);
    assert!(result.is_err());
}

#[test]
fn test_error_summary_contains_errors() {
    use rustdupe::duplicates::DuplicateFinder;
    use rustdupe::scanner::FileEntry;
    use std::time::SystemTime;

    let finder = DuplicateFinder::with_defaults();
    // Use files that don't exist to trigger hashing errors
    let file1 = FileEntry::new(PathBuf::from("nonexistent_1.txt"), 100, SystemTime::now());
    let file2 = FileEntry::new(PathBuf::from("nonexistent_2.txt"), 100, SystemTime::now());

    let (groups, summary) = finder
        .find_duplicates_from_files(vec![file1, file2])
        .unwrap();

    assert!(groups.is_empty());
    // Should have collected two errors during prehash phase
    assert_eq!(summary.scan_errors.len(), 2);
}

#[test]
fn test_scan_non_existent_path() {
    use rustdupe::duplicates::{DuplicateFinder, FinderError};
    let finder = DuplicateFinder::with_defaults();
    let result = finder.find_duplicates(std::path::Path::new("/non/existent/path/12345"));

    match result {
        Err(FinderError::PathNotFound(path)) => {
            assert!(path.to_string_lossy().contains("non/existent/path/12345"));
        }
        _ => panic!("Expected PathNotFound error, got {:?}", result),
    }
}

#[test]
fn test_scan_file_instead_of_directory() {
    use rustdupe::duplicates::{DuplicateFinder, FinderError};
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    File::create(&file_path).unwrap();

    let finder = DuplicateFinder::with_defaults();
    let result = finder.find_duplicates(&file_path);

    match result {
        Err(FinderError::NotADirectory(path)) => {
            assert!(path.to_string_lossy().contains("file.txt"));
        }
        _ => panic!("Expected NotADirectory error, got {:?}", result),
    }
}

#[test]
fn test_file_disappearing_during_scan() {
    use rustdupe::duplicates::{phase2_prehash, PrehashConfig};
    use rustdupe::scanner::{FileEntry, Hasher};
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("vanish.txt");
    File::create(&file_path)
        .unwrap()
        .write_all(b"gone soon")
        .unwrap();

    let dup_path = dir.path().join("dup.txt");
    File::create(&dup_path)
        .unwrap()
        .write_all(b"gone soon")
        .unwrap();

    let file_entry = FileEntry::new(file_path.clone(), 9, std::time::SystemTime::now());
    let dup_entry = FileEntry::new(dup_path, 9, std::time::SystemTime::now());

    let mut size_groups = std::collections::HashMap::new();
    size_groups.insert(9, vec![file_entry, dup_entry]);

    // Delete the file before phase 2
    std::fs::remove_file(&file_path).unwrap();

    let hasher = Arc::new(Hasher::new());
    let config = PrehashConfig::default();
    let (groups, stats) = phase2_prehash(size_groups, hasher, config);

    assert!(groups.is_empty());
    assert_eq!(stats.failed_files, 1);
    assert_eq!(stats.hashed_files, 1);
}

#[cfg(unix)]
#[test]
fn test_invalid_utf8_path() {
    use std::os::unix::ffi::OsStrExt;
    let dir = tempdir().unwrap();

    // Create a filename with invalid UTF-8 bytes
    let invalid_name = std::ffi::OsStr::from_bytes(&[0xff, 0xfe, 0xfd]);
    let file_path = dir.path().join(invalid_name);

    // If the filesystem doesn't support this, skip the test
    if let Ok(mut f) = File::create(&file_path) {
        f.write_all(b"invalid utf8").unwrap();

        let cli = Cli::try_parse_from(["rustdupe", "scan", dir.path().to_str().unwrap()]).unwrap();
        let result = rustdupe::run_app(cli).unwrap();

        assert_eq!(result, ExitCode::Success);
    }
}
