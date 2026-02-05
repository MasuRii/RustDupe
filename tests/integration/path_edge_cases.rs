use rustdupe::duplicates::DuplicateFinder;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;
use unicode_normalization::UnicodeNormalization;

#[test]
fn test_paths_with_quotes() {
    let dir = tempdir().unwrap();

    // Windows does not allow double quotes in filenames.
    if cfg!(not(windows)) {
        let quote_name = "file_with_\"quote\".txt";
        let file_path = dir.path().join(quote_name);

        File::create(&file_path)
            .expect("Failed to create file with quotes")
            .write_all(b"content")
            .unwrap();

        let dup_path = dir.path().join("duplicate.txt");
        File::create(&dup_path)
            .unwrap()
            .write_all(b"content")
            .unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (groups, _) = finder.find_duplicates(dir.path()).unwrap();

        assert_eq!(groups.len(), 1);
        assert!(groups[0]
            .files
            .iter()
            .any(|f| f.path.to_string_lossy().contains("\"")));
    }
}

#[test]
fn test_paths_with_newlines() {
    let dir = tempdir().unwrap();

    // Windows does not allow newlines in filenames.
    if cfg!(not(windows)) {
        let newline_name = "file_with\nnewline.txt";
        let file_path = dir.path().join(newline_name);

        File::create(&file_path)
            .expect("Failed to create file with newline")
            .write_all(b"content")
            .unwrap();

        let dup_path = dir.path().join("duplicate.txt");
        File::create(&dup_path)
            .unwrap()
            .write_all(b"content")
            .unwrap();

        let finder = DuplicateFinder::with_defaults();
        let (groups, _) = finder.find_duplicates(dir.path()).unwrap();

        assert_eq!(groups.len(), 1);
        assert!(groups[0]
            .files
            .iter()
            .any(|f| f.path.to_string_lossy().contains('\n')));
    }
}

#[test]
fn test_extremely_long_paths() {
    let dir = tempdir().unwrap();

    let mut current_path = dir.path().to_path_buf();
    let folder_name = "a".repeat(50);

    // Create a path that exceeds 260 characters if possible.
    // We'll use 6 levels of 50-char folders = 300+ chars.
    for i in 0..6 {
        current_path = current_path.join(format!("{}_{}", i, folder_name));
        if let Err(e) = fs::create_dir(&current_path) {
            eprintln!(
                "Skipping extremely long path test: failed to create dir: {}",
                e
            );
            return;
        }
    }

    let file_path = current_path.join("file.txt");
    if let Err(e) = File::create(&file_path).and_then(|mut f| f.write_all(b"content")) {
        eprintln!(
            "Skipping extremely long path test: failed to create file: {}",
            e
        );
        return;
    }

    let dup_path = dir.path().join("duplicate.txt");
    File::create(&dup_path)
        .unwrap()
        .write_all(b"content")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, _) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
}

#[test]
fn test_unicode_nfd_normalization_integration() {
    let dir = tempdir().unwrap();

    // "café" in NFC
    let name_nfc = "café_test.txt";
    // "café" in NFD
    let name_nfd = "cafe\u{0301}_test.txt";

    // Sanity check they are different strings
    assert_ne!(name_nfc, name_nfd);
    assert_eq!(name_nfc, name_nfd.nfc().collect::<String>());

    let path_nfc = dir.path().join(name_nfc);
    File::create(&path_nfc)
        .unwrap()
        .write_all(b"content")
        .unwrap();

    let path_nfd = dir.path().join(name_nfd);
    let nfd_write_result = File::create(&path_nfd).and_then(|mut f| f.write_all(b"content"));

    if let Err(e) = nfd_write_result {
        log::debug!(
            "Could not create second file (likely filesystem normalization): {}",
            e
        );
    }

    // Check how many files actually exist in the directory.
    // On macOS (APFS/HFS+), NFC and NFD are treated as the same filename,
    // so the second create may succeed but actually overwrite the first file.
    let file_count = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .count();

    let finder = DuplicateFinder::with_defaults();
    let (groups, _) = finder.find_duplicates(dir.path()).unwrap();

    if file_count == 2 {
        // If we actually have two files, they should be grouped together as duplicates.
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    } else {
        // Only one file exists (filesystem normalized the names).
        // This is expected on macOS with APFS/HFS+.
        // If we couldn't create both, then we only have one file.
        // Create another one with a different name to test grouping.
        let path_other = dir.path().join("other_file.txt");
        File::create(&path_other)
            .unwrap()
            .write_all(b"content")
            .unwrap();

        let (groups, _) = finder.find_duplicates(dir.path()).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
    }
}
