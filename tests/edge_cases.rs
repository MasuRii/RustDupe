use rustdupe::duplicates::DuplicateFinder;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_empty_files_skipped() {
    let dir = tempdir().unwrap();

    // Create two empty files
    File::create(dir.path().join("empty1.txt")).unwrap();
    File::create(dir.path().join("empty2.txt")).unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, _summary) = finder.find_duplicates(dir.path()).unwrap();

    // Empty files should be skipped by the scanner/finder logic per prd 3.3.1
    assert!(groups.is_empty());
    // Summary might show them as scanned but not duplicates, or they might be filtered out early.
    // prd 3.3.1 notes: \"Empty files (size 0) are skipped with warning logged\"
    // In my previous reading of 3.2.1 notes: \"Empty file skipping with debug logging\"
    // Let's verify what the summary shows.
}

#[test]
fn test_very_small_files() {
    let dir = tempdir().unwrap();

    // Create two files with 1 byte
    File::create(dir.path().join("small1.txt"))
        .unwrap()
        .write_all(b"a")
        .unwrap();
    File::create(dir.path().join("small2.txt"))
        .unwrap()
        .write_all(b"a")
        .unwrap();
    File::create(dir.path().join("small3.txt"))
        .unwrap()
        .write_all(b"b")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].size, 1);
    assert_eq!(groups[0].files.len(), 2);
    assert_eq!(summary.total_files, 3);
}

#[test]
fn test_file_at_prehash_boundary() {
    let dir = tempdir().unwrap();
    const PREHASH_SIZE: usize = 4096;

    let mut content1 = vec![b'x'; PREHASH_SIZE];
    let content2 = content1.clone();

    // Exactly 4KB duplicates
    File::create(dir.path().join("boundary1.txt"))
        .unwrap()
        .write_all(&content1)
        .unwrap();
    File::create(dir.path().join("boundary2.txt"))
        .unwrap()
        .write_all(&content2)
        .unwrap();

    // Exactly 4KB but different at the very end (last byte)
    content1[PREHASH_SIZE - 1] = b'y';
    File::create(dir.path().join("boundary3.txt"))
        .unwrap()
        .write_all(&content1)
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].size, PREHASH_SIZE as u64);
    assert_eq!(groups[0].files.len(), 2);
    assert_eq!(summary.total_files, 3);
}

#[test]
fn test_special_characters_in_filenames() {
    let dir = tempdir().unwrap();

    // Filename with spaces
    let space_name = "file with spaces.txt";
    File::create(dir.path().join(space_name))
        .unwrap()
        .write_all(b"content")
        .unwrap();
    File::create(dir.path().join("duplicate1.txt"))
        .unwrap()
        .write_all(b"content")
        .unwrap();

    // Filename with unicode
    let unicode_name = "café_🦀.txt";
    File::create(dir.path().join(unicode_name))
        .unwrap()
        .write_all(b"unicode content")
        .unwrap();
    File::create(dir.path().join("duplicate2.txt"))
        .unwrap()
        .write_all(b"unicode content")
        .unwrap();

    // Filename with special characters
    let special_name = "special_!@#$%^&()_+.txt";
    File::create(dir.path().join(special_name))
        .unwrap()
        .write_all(b"special content")
        .unwrap();
    File::create(dir.path().join("duplicate3.txt"))
        .unwrap()
        .write_all(b"special content")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, _summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 3);
}

#[test]
fn test_deeply_nested_paths() {
    let dir = tempdir().unwrap();
    let mut current_path = dir.path().to_path_buf();

    for i in 0..15 {
        current_path = current_path.join(format!("level_{}", i));
        fs::create_dir(&current_path).unwrap();
    }

    let file1 = current_path.join("deep.txt");
    File::create(&file1)
        .unwrap()
        .write_all(b"deep content")
        .unwrap();

    let file2 = dir.path().join("shallow.txt");
    File::create(&file2)
        .unwrap()
        .write_all(b"deep content")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(summary.total_files, 2);
}

#[test]
fn test_long_path_near_limit() {
    let dir = tempdir().unwrap();

    // Windows MAX_PATH is 260. We try to get close or exceed it.
    // The manifest should handle it if longPathAware is true.
    let long_name = "a".repeat(50);
    let mut current_path = dir.path().to_path_buf();

    // Create a path that is quite long
    for i in 0..4 {
        current_path = current_path.join(format!("{}_{}", i, long_name));
        if let Err(e) = fs::create_dir(&current_path) {
            eprintln!(
                "Failed to create dir at level {}: {}. Skipping long path test.",
                i, e
            );
            return;
        }
    }

    let file_path = current_path.join("file.txt");
    if let Err(e) = File::create(&file_path).map(|mut f| f.write_all(b"content")) {
        eprintln!(
            "Failed to create file in long path: {}. Skipping long path test.",
            e
        );
        return;
    }

    let file_path2 = dir.path().join("duplicate.txt");
    File::create(&file_path2)
        .unwrap()
        .write_all(b"content")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(summary.total_files, 2);
}
