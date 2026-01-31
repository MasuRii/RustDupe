use rustdupe::duplicates::{DuplicateFinder, FinderError};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_scan_non_existent_path() {
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

#[cfg(unix)]
#[test]
fn test_permission_denied_continues() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let sub = dir.path().join("no_access");
    fs::create_dir(&sub).unwrap();

    // Create a file in the directory before locking it
    let file_path = sub.join("hidden.txt");
    File::create(&file_path)
        .unwrap()
        .write_all(b"secret")
        .unwrap();

    // Remove read/execute permissions from directory
    let mut perms = fs::metadata(&sub).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&sub, perms).unwrap();

    // Create another accessible file
    let ok_file = dir.path().join("ok.txt");
    File::create(&ok_file)
        .unwrap()
        .write_all(b"public")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // The scan should continue and find ok.txt, but skip no_access
    assert_eq!(summary.total_files, 1);
    assert!(groups.is_empty());

    // Cleanup: restore permissions so tempdir can be deleted
    let mut perms = fs::metadata(&sub).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&sub, perms).unwrap();
}

#[test]
fn test_file_disappearing_during_scan() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("vanish.txt");
    File::create(&file_path)
        .unwrap()
        .write_all(b"gone soon")
        .unwrap();

    // Create a duplicate so it enters hashing phases
    let dup_path = dir.path().join("dup.txt");
    File::create(&dup_path)
        .unwrap()
        .write_all(b"gone soon")
        .unwrap();

    // We can't easily hook into the finder to delete the file between phases,
    // but we can test the phase2/phase3 functions directly or use a custom walker.

    use rustdupe::duplicates::{phase2_prehash, PrehashConfig};
    use rustdupe::scanner::{FileEntry, Hasher};
    use std::sync::Arc;

    let file_entry = FileEntry::new(file_path.clone(), 9, std::time::SystemTime::now());
    let dup_entry = FileEntry::new(dup_path, 9, std::time::SystemTime::now());

    let mut size_groups = std::collections::HashMap::new();
    size_groups.insert(9, vec![file_entry, dup_entry]);

    // Delete the file before phase 2
    fs::remove_file(&file_path).unwrap();

    let hasher = Arc::new(Hasher::new());
    let config = PrehashConfig::default();
    let (groups, stats) = phase2_prehash(size_groups, hasher, config);

    // One file failed, the other became unique, so no groups
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

        let finder = DuplicateFinder::with_defaults();
        let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

        assert_eq!(summary.total_files, 1);
        assert!(groups.is_empty());
    }
}
