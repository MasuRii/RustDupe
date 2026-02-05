use rustdupe::duplicates::{DuplicateFinder, FinderConfig, FinderError};
use rustdupe::scanner::{FileEntry, HashError, ScanError};
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn test_find_duplicates_from_files_continues_on_error() {
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

    for err in &summary.scan_errors {
        match err {
            ScanError::HashError(HashError::NotFound(_)) => {}
            _ => panic!("Expected NotFound HashError, got: {:?}", err),
        }
    }
}

#[test]
fn test_find_duplicates_from_files_strict_fails() {
    let config = FinderConfig::default().with_strict(true);
    let finder = DuplicateFinder::new(config);
    // Use two files with same size to trigger Phase 2 (hashing)
    let file1 = FileEntry::new(PathBuf::from("nonexistent_1.txt"), 100, SystemTime::now());
    let file2 = FileEntry::new(PathBuf::from("nonexistent_2.txt"), 100, SystemTime::now());

    let result = finder.find_duplicates_from_files(vec![file1, file2]);

    assert!(result.is_err());
    match result.unwrap_err() {
        FinderError::ScanError(ScanError::HashError(HashError::NotFound(_))) => {}
        other => panic!("Expected NotFound ScanError, got: {:?}", other),
    }
}
