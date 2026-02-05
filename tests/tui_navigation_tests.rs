use rustdupe::duplicates::DuplicateGroup;
use rustdupe::tui::app::{Action, App, AppMode, BulkSelectionType, SortColumn, SortDirection};
use std::path::PathBuf;

fn make_group(size: u64, paths: Vec<&str>) -> DuplicateGroup {
    let now = std::time::SystemTime::now();
    let mut hash = [0u8; 32];
    let size_bytes = size.to_be_bytes();
    hash[..8].copy_from_slice(&size_bytes);

    DuplicateGroup::new(
        hash,
        size,
        paths
            .into_iter()
            .map(|p| {
                let mut entry = rustdupe::scanner::FileEntry::new(PathBuf::from(p), size, now);
                // Assign group names for testing
                if p.contains("dir1") {
                    entry.group_name = Some("dir1".to_string());
                } else if p.contains("dir2") {
                    entry.group_name = Some("dir2".to_string());
                }
                entry
            })
            .collect(),
        Vec::new(),
    )
}

#[test]
fn test_search_filtering() {
    let groups = vec![
        make_group(1000, vec!["/dir1/file1.txt", "/dir2/file1_copy.txt"]),
        make_group(2000, vec!["/dir1/photo.jpg", "/dir2/photo_copy.jpg"]),
        make_group(3000, vec!["/dir1/data.csv", "/dir2/data_copy.csv"]),
    ];
    let mut app = App::with_groups(groups);

    // Default: all 3 groups visible
    assert_eq!(app.visible_group_count(), 3);

    // Filter by "photo"
    app.set_search_query("photo".to_string());
    assert_eq!(app.visible_group_count(), 1);
    assert_eq!(app.visible_group_at(0).unwrap().size, 2000);

    // Filter by "dir1" (all groups match)
    app.set_search_query("dir1".to_string());
    assert_eq!(app.visible_group_count(), 3);

    // Filter by "dir2" (all groups match)
    app.set_search_query("dir2".to_string());
    assert_eq!(app.visible_group_count(), 3);

    // Filter by group name (named "dir1" and "dir2" in make_group)
    app.set_search_query("dir1".to_string());
    assert_eq!(app.visible_group_count(), 3);

    // Clear search
    app.clear_search();
    assert_eq!(app.visible_group_count(), 3);
}

#[test]
fn test_search_regex() {
    let groups = vec![
        make_group(1000, vec!["/dir1/file1.txt", "/dir2/file1_copy.txt"]),
        make_group(2000, vec!["/dir1/photo.jpg", "/dir2/photo_copy.jpg"]),
        make_group(3000, vec!["/dir1/data.csv", "/dir2/data_copy.csv"]),
    ];
    let mut app = App::with_groups(groups);

    // Regex: match .jpg or .csv
    app.set_search_query(r"\.(jpg|csv)$".to_string());
    assert_eq!(app.visible_group_count(), 2);

    // Regex: match file1
    app.set_search_query("file1".to_string());
    assert_eq!(app.visible_group_count(), 1);
}

#[test]
fn test_bulk_selection_confirm_apply() {
    let groups = vec![
        make_group(1000, vec!["/a/1.txt", "/b/1.txt"]),
        make_group(2000, vec!["/a/2.txt", "/b/2.txt"]),
    ];
    let mut app = App::with_groups(groups);

    // Select all duplicates
    app.handle_action(Action::SelectAllDuplicates);
    assert_eq!(app.mode(), AppMode::ConfirmingBulkSelection);
    assert_eq!(app.pending_selection_count(), 2);
    assert_eq!(
        app.pending_bulk_action(),
        Some(BulkSelectionType::AllDuplicates)
    );

    // Confirm
    app.handle_action(Action::Confirm);
    assert_eq!(app.mode(), AppMode::Reviewing);
    assert_eq!(app.selected_count(), 2);
    assert!(app.is_file_selected(&PathBuf::from("/b/1.txt")));
    assert!(app.is_file_selected(&PathBuf::from("/b/2.txt")));
}

#[test]
fn test_bulk_selection_undo() {
    let groups = vec![
        make_group(1000, vec!["/a/1.txt", "/b/1.txt"]),
        make_group(2000, vec!["/a/2.txt", "/b/2.txt"]),
    ];
    let mut app = App::with_groups(groups);

    // Initial selection
    app.handle_action(Action::SelectAllDuplicates);
    app.handle_action(Action::Confirm);
    assert_eq!(app.selected_count(), 2);

    // Undo
    app.handle_action(Action::UndoSelection);
    assert_eq!(app.selected_count(), 0);
}

#[test]
fn test_expand_collapse_logic() {
    let groups = vec![
        make_group(1000, vec!["/a/1.txt", "/b/1.txt"]),
        make_group(2000, vec!["/a/2.txt", "/b/2.txt"]),
    ];
    let mut app = App::with_groups(groups);

    // Groups are collapsed by default
    assert!(!app.is_expanded(&app.groups()[0].hash));
    assert!(!app.is_expanded(&app.groups()[1].hash));

    // Expand current group (first one)
    app.handle_action(Action::ToggleExpand);
    assert!(app.is_expanded(&app.groups()[0].hash));
    assert!(!app.is_expanded(&app.groups()[1].hash));

    // Expand all
    app.handle_action(Action::ExpandAll);
    assert!(app.is_expanded(&app.groups()[0].hash));
    assert!(app.is_expanded(&app.groups()[1].hash));

    // Collapse all
    app.handle_action(Action::CollapseAll);
    assert!(!app.is_expanded(&app.groups()[0].hash));
    assert!(!app.is_expanded(&app.groups()[1].hash));

    // Toggle expand all
    app.handle_action(Action::ToggleExpandAll);
    assert!(app.is_expanded(&app.groups()[0].hash));
    assert!(app.is_expanded(&app.groups()[1].hash));

    app.handle_action(Action::ToggleExpandAll);
    assert!(!app.is_expanded(&app.groups()[0].hash));
    assert!(!app.is_expanded(&app.groups()[1].hash));
}

#[test]
fn test_sorting_cycling() {
    let groups = vec![
        make_group(3000, vec!["/z/file.txt", "/z/copy.txt"]),
        make_group(1000, vec!["/a/file.txt", "/a/copy.txt"]),
        make_group(2000, vec!["/m/file.txt", "/m/copy.txt"]),
    ];
    let mut app = App::with_groups(groups);

    // Default sort: Size Descending
    assert_eq!(app.sort_column(), SortColumn::Size);
    assert_eq!(app.sort_direction(), SortDirection::Descending);
    assert_eq!(app.visible_group_at(0).unwrap().size, 3000);
    assert_eq!(app.visible_group_at(1).unwrap().size, 2000);
    assert_eq!(app.visible_group_at(2).unwrap().size, 1000);

    // Reverse direction
    app.handle_action(Action::ReverseSortDirection);
    assert_eq!(app.sort_direction(), SortDirection::Ascending);
    assert_eq!(app.visible_group_at(0).unwrap().size, 1000);
    assert_eq!(app.visible_group_at(1).unwrap().size, 2000);
    assert_eq!(app.visible_group_at(2).unwrap().size, 3000);

    // Cycle to Path
    app.handle_action(Action::CycleSortColumn);
    assert_eq!(app.sort_column(), SortColumn::Path);
    // Path sorting is alphabetical. dir /a/ comes before /m/ comes before /z/
    // Since we are in Ascending direction:
    assert_eq!(
        app.visible_group_at(0).unwrap().files[0]
            .path
            .to_str()
            .unwrap(),
        "/a/file.txt"
    );
    assert_eq!(
        app.visible_group_at(1).unwrap().files[0]
            .path
            .to_str()
            .unwrap(),
        "/m/file.txt"
    );
    assert_eq!(
        app.visible_group_at(2).unwrap().files[0]
            .path
            .to_str()
            .unwrap(),
        "/z/file.txt"
    );
}

#[test]
fn test_navigation_with_filtering() {
    let groups = vec![
        make_group(1000, vec!["/dir1/a.txt", "/dir2/a.txt"]),
        make_group(2000, vec!["/dir1/b.txt", "/dir2/b.txt"]),
        make_group(3000, vec!["/dir1/c.txt", "/dir2/c.txt"]),
    ];
    let mut app = App::with_groups(groups);

    // Filter to b and c
    app.set_search_query("[bc].txt".to_string());
    assert_eq!(app.visible_group_count(), 2);

    // Navigate down
    app.next(); // Moves to next group because it's collapsed
    assert_eq!(app.group_index(), 1);
    assert_eq!(app.visible_group_at(app.group_index()).unwrap().size, 2000);

    // Navigate up
    app.previous();
    assert_eq!(app.group_index(), 0);
    assert_eq!(app.visible_group_at(app.group_index()).unwrap().size, 3000);
}
