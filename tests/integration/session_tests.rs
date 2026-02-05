use rustdupe::duplicates::DuplicateFinder;
use rustdupe::session::{Session, SessionGroup, SessionSettings};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_session_workflow_full() {
    let dir = tempdir().unwrap();
    let scan_path = dir.path().to_path_buf();

    // Create some duplicates
    let file1 = scan_path.join("file1.txt");
    let file2 = scan_path.join("file2.txt");
    let file3 = scan_path.join("file3.txt");

    File::create(&file1)
        .unwrap()
        .write_all(b"duplicate content")
        .unwrap();
    File::create(&file2)
        .unwrap()
        .write_all(b"duplicate content")
        .unwrap();
    File::create(&file3)
        .unwrap()
        .write_all(b"unique content")
        .unwrap();

    // 1. Scan
    let finder = DuplicateFinder::with_defaults();
    let (groups, _summary) = finder.find_duplicates(&scan_path).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);

    // 2. Convert to session
    let settings = SessionSettings::default();
    let session_groups = groups
        .iter()
        .enumerate()
        .map(|(i, g)| SessionGroup::from_duplicate_group(g, i))
        .collect();

    let mut session = Session::new(vec![scan_path.clone()], settings, session_groups);

    // 3. Add user selections
    let dup_file = &groups[0].files[1].path;
    session.user_selections.insert(dup_file.clone());
    session.group_index = 0;
    session.file_index = 1;

    // 4. Save session
    let session_path = dir.path().join("session.json");
    session.save(&session_path).unwrap();

    // 5. Load session
    let loaded = Session::load(&session_path).unwrap();

    // 6. Verify
    assert_eq!(loaded.scan_paths, vec![scan_path]);
    assert_eq!(loaded.groups.len(), 1);
    assert_eq!(loaded.groups[0].files.len(), 2);
    assert!(loaded.user_selections.contains(dup_file));
    assert_eq!(loaded.group_index, 0);
    assert_eq!(loaded.file_index, 1);
}

#[test]
fn test_session_load_with_missing_files() {
    let dir = tempdir().unwrap();
    let session_path = dir.path().join("session.json");

    // Create a session referencing a file that we will delete
    let file_to_delete = dir.path().join("to_delete.txt");
    File::create(&file_to_delete)
        .unwrap()
        .write_all(b"content")
        .unwrap();

    let now = std::time::SystemTime::now();
    let group = SessionGroup {
        id: 1,
        hash: [0u8; 32],
        size: 7,
        files: vec![rustdupe::scanner::FileEntry::new(
            file_to_delete.clone(),
            7,
            now,
        )],
        reference_paths: Vec::new(),
        is_similar: false,
    };

    let session = Session::new(
        vec![dir.path().to_path_buf()],
        SessionSettings::default(),
        vec![group],
    );
    session.save(&session_path).unwrap();

    // Delete the file
    fs::remove_file(&file_to_delete).unwrap();

    // Loading should still succeed (with a warning)
    let loaded = Session::load(&session_path).unwrap();
    assert_eq!(loaded.groups.len(), 1);
    assert_eq!(loaded.groups[0].files[0].path, file_to_delete);
}

#[test]
fn test_session_app_integration() {
    let dir = tempdir().unwrap();
    let scan_path = dir.path().to_path_buf();

    // Create some duplicates
    let file1 = scan_path.join("file1.txt");
    let file2 = scan_path.join("file2.txt");

    File::create(&file1).unwrap().write_all(b"content").unwrap();
    File::create(&file2).unwrap().write_all(b"content").unwrap();

    let finder = DuplicateFinder::with_defaults();
    let (groups, _summary) = finder.find_duplicates(&scan_path).unwrap();

    // Create session
    let session_groups: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| SessionGroup::from_duplicate_group(g, i))
        .collect();
    let mut session = Session::new(
        vec![scan_path.clone()],
        SessionSettings::default(),
        session_groups,
    );
    session.user_selections.insert(file2.clone());
    session.group_index = 0;
    session.file_index = 1;

    // Apply to App
    let (dup_groups, _summary) = session.to_results();
    let mut app = rustdupe::tui::app::App::with_groups(dup_groups);
    app.apply_session(
        session.user_selections.clone(),
        session.group_index,
        session.file_index,
    );

    // Verify App state
    assert!(app.is_file_selected(&file2));
    assert_eq!(app.group_index(), 0);
    assert_eq!(app.file_index(), 1);
}
