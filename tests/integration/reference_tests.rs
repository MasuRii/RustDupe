use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::tui::app::App;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_scan_with_reference_flag() {
    let dir = tempdir().unwrap();
    let ref_dir = dir.path().join("reference");
    let data_dir = dir.path().join("data");

    fs::create_dir(&ref_dir).unwrap();
    fs::create_dir(&data_dir).unwrap();

    let content = b"duplicate content";

    // Create original in reference dir
    let ref_file = ref_dir.join("original.txt");
    File::create(&ref_file).unwrap().write_all(content).unwrap();

    // Create duplicate in data dir
    let dup_file = data_dir.join("duplicate.txt");
    File::create(&dup_file).unwrap().write_all(content).unwrap();

    // Canonicalize for reliable matching
    let canon_ref_dir = ref_dir.canonicalize().unwrap();
    let canon_ref_file = ref_file.canonicalize().unwrap();
    let canon_dup_file = dup_file.canonicalize().unwrap();

    let finder_config = FinderConfig::default().with_reference_paths(vec![canon_ref_dir.clone()]);
    let finder = DuplicateFinder::new(finder_config);

    let scan_root = dir.path().canonicalize().unwrap();
    let (groups, summary) = finder.find_duplicates(&scan_root).unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.files.len(), 2);

    // Verify reference file is correctly identified in the group
    let ref_entry = group
        .files
        .iter()
        .find(|f| f.path.canonicalize().unwrap() == canon_ref_file)
        .expect("Reference file not found in group");
    let dup_entry = group
        .files
        .iter()
        .find(|f| f.path.canonicalize().unwrap() == canon_dup_file)
        .expect("Duplicate file not found in group");

    assert!(group.is_in_reference_dir(&ref_entry.path));
    assert!(!group.is_in_reference_dir(&dup_entry.path));

    assert_eq!(summary.duplicate_groups, 1);
}

#[test]
fn test_reference_tui_integration() {
    let dir = tempdir().unwrap();
    let ref_dir = dir.path().join("reference");
    let data_dir = dir.path().join("data");

    fs::create_dir(&ref_dir).unwrap();
    fs::create_dir(&data_dir).unwrap();

    let content = b"duplicate content";

    let ref_file = ref_dir.join("original.txt");
    File::create(&ref_file).unwrap().write_all(content).unwrap();

    let dup_file1 = data_dir.join("dup1.txt");
    File::create(&dup_file1)
        .unwrap()
        .write_all(content)
        .unwrap();

    let dup_file2 = data_dir.join("dup2.txt");
    File::create(&dup_file2)
        .unwrap()
        .write_all(content)
        .unwrap();

    let canon_ref_dir = ref_dir.canonicalize().unwrap();

    let finder_config = FinderConfig::default().with_reference_paths(vec![canon_ref_dir.clone()]);
    let finder = DuplicateFinder::new(finder_config);

    let scan_root = dir.path().canonicalize().unwrap();
    let (groups, _) = finder.find_duplicates(&scan_root).unwrap();

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![canon_ref_dir]);

    // Test select all in group
    app.select_all_in_group();

    // Should NOT select the reference file
    let canon_ref_file = ref_file.canonicalize().unwrap();
    for file in app.groups()[0].files.iter() {
        if file.path.canonicalize().unwrap() == canon_ref_file {
            assert!(!app.is_file_selected(&file.path));
        }
    }
}

#[test]
fn test_reference_directory_with_subdirectories_integration() {
    let dir = tempdir().unwrap();
    let ref_dir = dir.path().join("reference");
    let ref_sub_dir = ref_dir.join("sub").join("dir");
    let data_dir = dir.path().join("data");

    fs::create_dir_all(&ref_sub_dir).unwrap();
    fs::create_dir(&data_dir).unwrap();

    let content = b"duplicate content";

    let ref_file = ref_sub_dir.join("original.txt");
    File::create(&ref_file).unwrap().write_all(content).unwrap();

    let dup_file = data_dir.join("duplicate.txt");
    File::create(&dup_file).unwrap().write_all(content).unwrap();

    let canon_ref_dir = ref_dir.canonicalize().unwrap();
    let canon_ref_file = ref_file.canonicalize().unwrap();

    let finder_config = FinderConfig::default().with_reference_paths(vec![canon_ref_dir.clone()]);
    let finder = DuplicateFinder::new(finder_config);

    let scan_root = dir.path().canonicalize().unwrap();
    let (groups, _) = finder.find_duplicates(&scan_root).unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    // Find ref entry
    let ref_entry = group
        .files
        .iter()
        .find(|f| f.path.canonicalize().unwrap() == canon_ref_file)
        .expect("Nested reference file not found in group");
    assert!(group.is_in_reference_dir(&ref_entry.path));
}

#[test]
fn test_multiple_reference_directories_integration() {
    let dir = tempfile::tempdir().unwrap();
    let ref_dir1 = dir.path().join("ref1");
    let ref_dir2 = dir.path().join("ref2");
    let data_dir = dir.path().join("data");

    fs::create_dir(&ref_dir1).unwrap();
    fs::create_dir(&ref_dir2).unwrap();
    fs::create_dir(&data_dir).unwrap();

    let content = b"duplicate content";

    let file1 = ref_dir1.join("file1.txt");
    File::create(&file1).unwrap().write_all(content).unwrap();

    let file2 = ref_dir2.join("file2.txt");
    File::create(&file2).unwrap().write_all(content).unwrap();

    let file3 = data_dir.join("file3.txt");
    File::create(&file3).unwrap().write_all(content).unwrap();

    let canon_ref1 = ref_dir1.canonicalize().unwrap();
    let canon_ref2 = ref_dir2.canonicalize().unwrap();

    let finder_config = FinderConfig::default().with_reference_paths(vec![canon_ref1, canon_ref2]);
    let finder = DuplicateFinder::new(finder_config);

    let scan_root = dir.path().canonicalize().unwrap();
    let (groups, _) = finder.find_duplicates(&scan_root).unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    let canon_file1 = file1.canonicalize().unwrap();
    let canon_file2 = file2.canonicalize().unwrap();
    let canon_file3 = file3.canonicalize().unwrap();

    for file in &group.files {
        let canon = file.path.canonicalize().unwrap();
        if canon == canon_file1 || canon == canon_file2 {
            assert!(group.is_in_reference_dir(&file.path));
        } else if canon == canon_file3 {
            assert!(!group.is_in_reference_dir(&file.path));
        }
    }
}

#[test]
fn test_first_path_is_reference_in_multi_path_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path1 = dir.path().join("path1");
    let path2 = dir.path().join("path2");

    fs::create_dir(&path1).unwrap();
    fs::create_dir(&path2).unwrap();

    let content = b"duplicate content";
    let file1 = path1.join("file1.txt");
    fs::write(&file1, content).unwrap();
    let file2 = path2.join("file2.txt");
    fs::write(&file2, content).unwrap();

    let canon_path1 = path1.canonicalize().unwrap();
    let canon_path2 = path2.canonicalize().unwrap();

    // Simulate main.rs logic: multiple paths -> first is reference
    let mut reference_paths = Vec::new();
    let scan_paths = vec![canon_path1.clone(), canon_path2];
    if scan_paths.len() > 1 {
        reference_paths.push(scan_paths[0].clone());
    }

    let finder_config = FinderConfig::default().with_reference_paths(reference_paths);
    let finder = DuplicateFinder::new(finder_config);

    let (groups, _) = finder.find_duplicates_in_paths(scan_paths).unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    // file1 is in path1 (the first path), so it should be a reference
    let entry1 = group
        .files
        .iter()
        .find(|f| f.path.ends_with("file1.txt"))
        .unwrap();
    let entry2 = group
        .files
        .iter()
        .find(|f| f.path.ends_with("file2.txt"))
        .unwrap();

    assert!(group.is_in_reference_dir(&entry1.path));
    assert!(!group.is_in_reference_dir(&entry2.path));
}
