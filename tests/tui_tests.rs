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

fn make_group(size: u64, paths: Vec<&str>) -> DuplicateGroup {
    let now = std::time::SystemTime::now();
    DuplicateGroup::new(
        [0u8; 32],
        size,
        paths
            .into_iter()
            .map(|p| rustdupe::scanner::FileEntry::new(PathBuf::from(p), size, now))
            .collect(),
        Vec::new(),
    )
}

#[test]
fn test_render_header() {
    let mut terminal = setup_terminal(80, 24);
    let app = App::new();

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Header should contain the title
    assert!(content.contains("rustdupe - Smart Duplicate Finder"));
    assert!(content.contains("Scanning..."));
}

#[test]
fn test_render_empty_state() {
    let mut terminal = setup_terminal(80, 24);
    let mut app = App::new();
    app.set_mode(AppMode::Reviewing);

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    assert!(content.contains("No duplicate files found."));
}

#[test]
fn test_render_file_list() {
    let mut terminal = setup_terminal(80, 24);
    let groups = vec![
        make_group(2000, vec!["file1.txt", "file1_copy.txt"]),
        make_group(1000, vec!["file2.txt", "file2_copy.txt"]),
    ];
    let mut app = App::with_groups(groups);
    app.handle_action(rustdupe::tui::app::Action::ToggleExpandAll);

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Should show group info
    assert!(content.contains("file1.txt"));
    assert!(content.contains("2 copies"));

    // Should show file list info
    assert!(content.contains("[*] file1.txt"));
    assert!(content.contains("[ ] file1_copy.txt"));
}

#[test]
fn test_render_selection_highlight() {
    let mut terminal = setup_terminal(80, 24);
    let groups = vec![make_group(1000, vec!["file1.txt", "file1_copy.txt"])];
    let mut app = App::with_groups(groups);
    app.handle_action(rustdupe::tui::app::Action::ToggleExpandAll);

    // Move to second file and select it
    app.next();
    app.toggle_select();

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Selected file should have [X]
    assert!(content.contains("[X] file1_copy.txt"));
}

#[test]
fn test_render_footer() {
    let mut terminal = setup_terminal(150, 24);
    let app = App::with_groups(vec![make_group(100, vec!["a", "b"])]);

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Footer should contain navigation hint (platform-specific: "↑↓/jk" on Windows, "jk/↑↓" on Unix)
    assert!(
        content.contains("jk") || content.contains("Nav"),
        "Footer should contain navigation hint"
    );
    assert!(content.contains("Nav"));
    assert!(content.contains("[d]"));
    assert!(content.contains("Del"));
}

#[test]
fn test_render_truncation() {
    let mut terminal = setup_terminal(40, 24); // Narrow terminal
    let long_path =
        "/very/long/path/to/some/deeply/nested/directory/structure/file_with_long_name.txt";
    let groups = vec![make_group(1000, vec![long_path, "copy.txt"])];
    let mut app = App::with_groups(groups);
    app.handle_action(rustdupe::tui::app::Action::ToggleExpandAll);

    terminal
        .draw(|f| {
            render(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = format!("{:?}", buffer);

    // Path should be truncated with ellipsis
    assert!(content.contains("..."));
}
