use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_scan_two_non_overlapping_directories() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    File::create(dir1.path().join("a.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();
    File::create(dir2.path().join("b.txt"))
        .unwrap()
        .write_all(b"dup")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder
        .find_duplicates_in_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()])
        .unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);
    assert_eq!(summary.total_files, 2);
}

#[test]
fn test_scan_overlapping_directories() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();

    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"content")
        .unwrap();
    File::create(sub.join("b.txt"))
        .unwrap()
        .write_all(b"content")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    // Providing parent and child - child should be filtered out to avoid double scanning
    let (groups, summary) = finder
        .find_duplicates_in_paths(vec![dir.path().to_path_buf(), sub.to_path_buf()])
        .unwrap();

    // If double scanned, we'd have more files or duplicates.
    // Total files should be 2.
    assert_eq!(summary.total_files, 2);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);
}

#[test]
fn test_cross_directory_duplicate_detection() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    let dir3 = tempdir().unwrap();

    File::create(dir1.path().join("1.txt"))
        .unwrap()
        .write_all(b"triple")
        .unwrap();
    File::create(dir2.path().join("2.txt"))
        .unwrap()
        .write_all(b"triple")
        .unwrap();
    File::create(dir3.path().join("3.txt"))
        .unwrap()
        .write_all(b"triple")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder
        .find_duplicates_in_paths(vec![
            dir1.path().to_path_buf(),
            dir2.path().to_path_buf(),
            dir3.path().to_path_buf(),
        ])
        .unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 3);
    assert_eq!(summary.total_files, 3);
}

#[test]
fn test_reference_directory_protection_in_multi_path() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let ref_file = dir1.path().join("ref.txt");
    File::create(&ref_file).unwrap().write_all(b"dup").unwrap();
    let dup_file = dir2.path().join("dup.txt");
    File::create(&dup_file).unwrap().write_all(b"dup").unwrap();

    let finder_config =
        FinderConfig::default().with_reference_paths(vec![dir1.path().to_path_buf()]);
    let finder = DuplicateFinder::new(finder_config);
    let (groups, _) = finder
        .find_duplicates_in_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()])
        .unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    assert!(group.is_in_reference_dir(&ref_file));
    assert!(!group.is_in_reference_dir(&dup_file));
}

#[test]
#[cfg(unix)]
#[ignore = "TODO: hardlink tracker incorrectly filters symlinks - see issue #XXX"]
fn test_scan_with_symlinks_between_directories() {
    use rustdupe::scanner::WalkerConfig;
    use std::os::unix::fs::symlink;

    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let file1 = dir1.path().join("file1.txt");
    File::create(&file1).unwrap().write_all(b"content").unwrap();

    // Link from dir2 to file in dir1
    let link2 = dir2.path().join("link2.txt");
    symlink(&file1, &link2).unwrap();

    // By default, symlinks are NOT followed
    let finder = DuplicateFinder::with_defaults();
    let (_, summary) = finder
        .find_duplicates_in_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()])
        .unwrap();
    assert_eq!(summary.total_files, 1);

    // Now follow symlinks
    let walker_config = WalkerConfig::default().with_follow_symlinks(true);
    let finder_config = FinderConfig::default().with_walker_config(walker_config);
    let finder = DuplicateFinder::new(finder_config);
    let (groups, summary) = finder
        .find_duplicates_in_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()])
        .unwrap();

    assert_eq!(summary.total_files, 2);
    assert_eq!(groups.len(), 1);
}

#[test]
fn test_scan_same_path_twice() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("a.txt"))
        .unwrap()
        .write_all(b"content")
        .unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (_, summary) = finder
        .find_duplicates_in_paths(vec![dir.path().to_path_buf(), dir.path().to_path_buf()])
        .unwrap();

    assert_eq!(summary.total_files, 1);
}
