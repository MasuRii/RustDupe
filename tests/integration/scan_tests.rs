use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::scanner::WalkerConfig;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_scan_empty_directory() {
    let dir = tempdir().unwrap();
    let finder = DuplicateFinder::with_defaults();

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert!(groups.is_empty());
    assert_eq!(summary.total_files, 0);
    assert_eq!(summary.duplicate_groups, 0);
}

#[test]
fn test_scan_unique_files() {
    let dir = tempdir().unwrap();

    // Create 3 unique files
    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"content a")
        .unwrap();
    File::create(dir.path().join("b.txt"))
        .unwrap()
        .write_all(b"content b")
        .unwrap();
    File::create(dir.path().join("c.txt"))
        .unwrap()
        .write_all(b"content c")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert!(groups.is_empty());
    assert_eq!(summary.total_files, 3);
    assert_eq!(summary.duplicate_groups, 0);
}

#[test]
fn test_scan_duplicate_files() {
    let dir = tempdir().unwrap();

    // Create 2 identical files and 1 unique
    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"duplicate")
        .unwrap();
    File::create(dir.path().join("b.txt"))
        .unwrap()
        .write_all(b"duplicate")
        .unwrap();
    File::create(dir.path().join("c.txt"))
        .unwrap()
        .write_all(b"unique")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);
    assert_eq!(summary.total_files, 3);
    assert_eq!(summary.duplicate_groups, 1);
    assert_eq!(summary.duplicate_files, 1);
}

#[test]
fn test_scan_nested_directories() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();

    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(sub.join("b.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);
    assert_eq!(summary.total_files, 2);
}

#[test]
fn test_scan_multiple_groups() {
    let dir = tempdir().unwrap();

    // Group 1: 3 files
    File::create(dir.path().join("1a.txt"))
        .unwrap()
        .write_all(b"group1")
        .unwrap();
    File::create(dir.path().join("1b.txt"))
        .unwrap()
        .write_all(b"group1")
        .unwrap();
    File::create(dir.path().join("1c.txt"))
        .unwrap()
        .write_all(b"group1")
        .unwrap();

    // Group 2: 2 files
    File::create(dir.path().join("2a.txt"))
        .unwrap()
        .write_all(b"group2")
        .unwrap();
    File::create(dir.path().join("2b.txt"))
        .unwrap()
        .write_all(b"group2")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 2);
    assert_eq!(summary.duplicate_groups, 2);
    assert_eq!(summary.duplicate_files, 3);
}

#[test]
fn test_scan_size_filtering() {
    let dir = tempdir().unwrap();

    // 10 byte file (duplicate)
    File::create(dir.path().join("10a.txt"))
        .unwrap()
        .write_all(b"0123456789")
        .unwrap();
    File::create(dir.path().join("10b.txt"))
        .unwrap()
        .write_all(b"0123456789")
        .unwrap();

    // 20 byte file (duplicate)
    File::create(dir.path().join("20a.txt"))
        .unwrap()
        .write_all(b"01234567890123456789")
        .unwrap();
    File::create(dir.path().join("20b.txt"))
        .unwrap()
        .write_all(b"01234567890123456789")
        .unwrap();

    // Scan with min_size = 15
    let walker_config = WalkerConfig::default().with_min_size(Some(15));
    let finder_config = FinderConfig::default().with_walker_config(walker_config);
    let finder = DuplicateFinder::new(finder_config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // Only the 20 byte files should be seen
    assert_eq!(summary.total_files, 2);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].size, 20);
}

#[test]
fn test_scan_multiple_ignore_patterns() {
    let dir = tempdir().unwrap();

    File::create(dir.path().join("test.tmp"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("test.log"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("keep.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("keep2.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    // Nested ignored file
    let ignored_dir = dir.path().join("ignored");
    fs::create_dir(&ignored_dir).unwrap();
    File::create(ignored_dir.join("file.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    let walker_config = WalkerConfig::default().with_patterns(vec![
        "*.tmp".to_string(),
        "*.log".to_string(),
        "ignored/**".to_string(),
    ]);
    let finder_config = FinderConfig::default().with_walker_config(walker_config);
    let finder = DuplicateFinder::new(finder_config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // Only keep.txt and keep2.txt should be seen
    assert_eq!(summary.total_files, 2);
    assert_eq!(groups.len(), 1);
}

#[test]
fn test_scan_regex_filtering() {
    use regex::Regex;
    let dir = tempdir().unwrap();

    File::create(dir.path().join("match_this.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("ignore_this.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir.path().join("exclude_this.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    // Include only files starting with "match" or "ignore"
    // But then exclude files containing "ignore"
    let walker_config = WalkerConfig::default()
        .with_regex_include(vec![Regex::new("^(match|ignore)").unwrap()])
        .with_regex_exclude(vec![Regex::new("ignore").unwrap()]);
    let finder_config = FinderConfig::default().with_walker_config(walker_config);
    let finder = DuplicateFinder::new(finder_config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // Only match_this.txt should be seen
    assert_eq!(summary.total_files, 1);
    assert!(groups.is_empty()); // No duplicates found (only 1 file seen)
}

#[test]
fn test_scan_file_type_filtering() {
    use rustdupe::scanner::FileCategory;
    let dir = tempdir().unwrap();

    File::create(dir.path().join("image.jpg"))
        .unwrap()
        .write_all(b"image")
        .unwrap();
    File::create(dir.path().join("doc.pdf"))
        .unwrap()
        .write_all(b"doc")
        .unwrap();
    File::create(dir.path().join("audio.mp3"))
        .unwrap()
        .write_all(b"audio")
        .unwrap();

    // Filter for images and documents
    let walker_config = WalkerConfig::default()
        .with_file_categories(vec![FileCategory::Images, FileCategory::Documents]);
    let finder_config = FinderConfig::default().with_walker_config(walker_config);
    let finder = DuplicateFinder::new(finder_config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // Should see image.jpg and doc.pdf
    assert_eq!(summary.total_files, 2);
    assert!(groups.is_empty());
}
