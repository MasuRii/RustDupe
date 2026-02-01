use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rustdupe::duplicates::DuplicateGroup;
use rustdupe::tui::app::App;
use rustdupe::tui::ui::render;
use std::path::{Path, PathBuf};

fn setup_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    Terminal::new(backend).unwrap()
}

fn make_group_with_refs(size: u64, paths: Vec<&str>, refs: Vec<&str>) -> DuplicateGroup {
    let now = std::time::SystemTime::now();
    DuplicateGroup::new(
        [0u8; 32],
        size,
        paths
            .into_iter()
            .map(|p| rustdupe::scanner::FileEntry::new(PathBuf::from(p), size, now))
            .collect(),
        refs.into_iter().map(PathBuf::from).collect(),
    )
}

#[test]
fn test_reference_visual_indicator() {
    let mut terminal = setup_terminal(80, 24);

    // Create a group where the second file is in a reference directory
    let groups = vec![make_group_with_refs(
        1000,
        vec!["/data/file1.txt", "/ref/file1_copy.txt"],
        vec!["/ref"],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![PathBuf::from("/ref")]);

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Should show [R] for the reference file
    assert!(
        content.contains("[R] /ref/file1_copy.txt"),
        "Buffer content: {}",
        content
    );
}

#[test]
fn test_prevent_selecting_reference_file() {
    let groups = vec![make_group_with_refs(
        1000,
        vec!["/data/file1.txt", "/ref/file1_copy.txt"],
        vec!["/ref"],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![PathBuf::from("/ref")]);

    // Navigate to the reference file
    app.next();
    assert_eq!(
        app.current_file().unwrap().to_str().unwrap(),
        "/ref/file1_copy.txt"
    );

    // Try to toggle select
    app.toggle_select();

    // Should NOT be selected
    assert!(!app.is_current_selected());
    assert_eq!(app.selected_count(), 0);

    // Should show error message
    assert!(app.error_message().is_some());
    assert!(app
        .error_message()
        .unwrap()
        .contains("protected reference directory"));
}

#[test]
fn test_select_all_skips_reference_files() {
    let groups = vec![make_group_with_refs(
        1000,
        vec![
            "/data/file1.txt",
            "/data/file1_copy.zip",
            "/ref/file1_copy.txt",
        ],
        vec!["/ref"],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![PathBuf::from("/ref")]);

    // Select all in group
    app.select_all_in_group();

    // Should select /data/file1_copy.zip but NOT /ref/file1_copy.txt
    // /data/file1.txt is skipped because it's the first file (original)
    assert!(!app.is_file_selected(&PathBuf::from("/data/file1.txt")));
    assert!(app.is_file_selected(&PathBuf::from("/data/file1_copy.zip")));
    assert!(!app.is_file_selected(&PathBuf::from("/ref/file1_copy.txt")));
    assert_eq!(app.selected_count(), 1);
}

#[test]
fn test_reference_directory_with_subdirectories() {
    let groups = vec![make_group_with_refs(
        1000,
        vec!["/data/file1.txt", "/ref/sub/dir/file1_copy.txt"],
        vec!["/ref"],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![PathBuf::from("/ref")]);

    // Should match subdirectory
    assert!(app.is_in_reference_dir(Path::new("/ref/sub/dir/file1_copy.txt")));
}

#[test]
fn test_multiple_reference_directories() {
    let groups = vec![make_group_with_refs(
        1000,
        vec!["/ref1/file1.txt", "/ref2/file2.txt", "/data/file3.txt"],
        vec!["/ref1", "/ref2"],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![PathBuf::from("/ref1"), PathBuf::from("/ref2")]);

    assert!(app.is_in_reference_dir(Path::new("/ref1/file1.txt")));
    assert!(app.is_in_reference_dir(Path::new("/ref2/file2.txt")));
    assert!(!app.is_in_reference_dir(Path::new("/data/file3.txt")));
}

#[test]
fn test_reference_path_with_symlinks() {
    use std::fs;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let ref_dir = dir.path().join("ref");
    let data_dir = dir.path().join("data");
    let link_dir = dir.path().join("link");

    fs::create_dir(&ref_dir).unwrap();
    fs::create_dir(&data_dir).unwrap();

    let ref_file = ref_dir.join("file.txt");
    fs::write(&ref_file, "content").unwrap();

    // Create a symlink to the reference directory
    #[cfg(unix)]
    std::os::unix::fs::symlink(&ref_dir, &link_dir).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&ref_dir, &link_dir).unwrap();

    let linked_file = link_dir.join("file.txt");

    // Canonicalize paths
    let canon_ref_dir = ref_dir.canonicalize().unwrap();
    let canon_linked_file = linked_file.canonicalize().unwrap();

    let groups = vec![make_group_with_refs(
        10,
        vec![ref_file.to_str().unwrap(), linked_file.to_str().unwrap()],
        vec![canon_ref_dir.to_str().unwrap()],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![canon_ref_dir]);

    // If we use canonicalized path, it should match
    assert!(app.is_in_reference_dir(&canon_linked_file));

    // If we use the raw linked path, it won't match unless we canonicalize in is_in_reference_dir
    // For now, let's see what the implementation does.
    // Based on the code, it does NOT canonicalize.
    assert!(!app.is_in_reference_dir(&linked_file));
}

#[test]
fn test_batch_selection_respects_references() {
    let now = std::time::SystemTime::now();
    let minute = std::time::Duration::from_secs(60);

    let file1 = rustdupe::scanner::FileEntry::new(PathBuf::from("/data/newest.txt"), 1000, now);
    let file2 =
        rustdupe::scanner::FileEntry::new(PathBuf::from("/ref/oldest.txt"), 1000, now - minute);
    let file3 = rustdupe::scanner::FileEntry::new(
        PathBuf::from("/data/middle.txt"),
        1000,
        now - (minute / 2),
    );

    let groups = vec![DuplicateGroup::new(
        [0u8; 32],
        1000,
        vec![file1.clone(), file2.clone(), file3.clone()],
        vec![PathBuf::from("/ref")],
    )];

    let mut app = App::with_groups(groups);
    app.set_reference_paths(vec![PathBuf::from("/ref")]);

    // Test Select All Duplicates
    app.select_all_duplicates();
    // Should NOT select /ref/oldest.txt
    assert!(!app.is_file_selected(&file2.path));
    // Should select /data/middle.txt (file3)
    assert!(app.is_file_selected(&file3.path));
    // /data/newest.txt is the first file, so it's kept as original
    assert!(!app.is_file_selected(&file1.path));

    app.deselect_all();

    // Test Select Oldest (Keep Newest)
    app.select_oldest();
    // newest is file1. file2 is oldest but it's in ref, so it should be skipped.
    // file3 is middle, should be selected.
    assert!(!app.is_file_selected(&file2.path));
    assert!(app.is_file_selected(&file3.path));
    assert!(!app.is_file_selected(&file1.path));

    app.deselect_all();

    // Test Select Newest (Keep Oldest)
    // Wait, if oldest is in ref, can we keep it?
    // Yes, keeping a file is just NOT selecting it.
    // If we select newest (file1), file2 is oldest (keep). file3 is middle (select).
    app.select_newest();
    assert!(app.is_file_selected(&file1.path));
    assert!(app.is_file_selected(&file3.path));
    assert!(!app.is_file_selected(&file2.path));
}
