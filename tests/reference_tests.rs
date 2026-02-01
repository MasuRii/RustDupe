use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rustdupe::duplicates::DuplicateGroup;
use rustdupe::tui::app::{App, AppMode};
use rustdupe::tui::ui::render;
use std::path::PathBuf;

fn setup_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    Terminal::new(backend).unwrap()
}

fn make_group_with_refs(size: u64, paths: Vec<&str>, refs: Vec<&str>) -> DuplicateGroup {
    DuplicateGroup::new(
        [0u8; 32],
        size,
        paths.into_iter().map(PathBuf::from).collect(),
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
