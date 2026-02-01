use rustdupe::session::{Session, SessionGroup, SessionSettings};
use std::fs::{self, File};
use tempfile::tempdir;

#[test]
fn test_load_invalid_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("invalid.json");
    fs::write(&path, "not a json").unwrap();

    let result = Session::load(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to parse session envelope"));
}

#[test]
fn test_load_empty_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.json");
    File::create(&path).unwrap();

    let result = Session::load(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_unknown_version() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("version.json");

    let session = Session::new(
        vec![dir.path().to_path_buf()],
        SessionSettings::default(),
        vec![],
    );

    // Manually create a session with an old version to bypass SESSION_VERSION check in new()
    let mut session_data = session.clone();
    session_data.version = 999;

    // We need to use to_json to get a valid envelope, but we want to override the version
    // The current Session::save doesn't allow overriding version easily if it's hardcoded to SESSION_VERSION in new
    // But Session struct fields are public, so we can just set it.

    session_data.save(&path).unwrap();

    let result = Session::load(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unsupported session version"));
    assert!(err.contains("999"));
}

#[test]
fn test_load_incorrect_integrity_hash() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("tampered.json");

    let session = Session::new(
        vec![dir.path().to_path_buf()],
        SessionSettings::default(),
        vec![],
    );
    session.save(&path).unwrap();

    // Manually tamper with the session data in the JSON
    let mut content = fs::read_to_string(&path).unwrap();
    // Change group_index: 0 to group_index: 100
    assert!(content.contains("\"group_index\": 0"));
    content = content.replace("\"group_index\": 0", "\"group_index\": 100");
    fs::write(&path, content).unwrap();

    let result = Session::load(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("integrity check failed"));
}

#[test]
fn test_load_missing_envelope_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing_fields.json");

    // Valid JSON but missing "session" or "checksum"
    fs::write(&path, "{\"checksum\": \"abc\"}").unwrap();

    let result = Session::load(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to parse session envelope"));
}

#[test]
fn test_load_referencing_deleted_files_integration() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.json");
    let file_path = dir.path().join("exists.txt");
    fs::write(&file_path, "content").unwrap();

    let now = std::time::SystemTime::now();
    let group = SessionGroup {
        id: 1,
        hash: [0u8; 32],
        size: 7,
        files: vec![rustdupe::scanner::FileEntry::new(file_path.clone(), 7, now)],
        reference_paths: Vec::new(),
    };

    let session = Session::new(
        vec![dir.path().to_path_buf()],
        SessionSettings::default(),
        vec![group],
    );
    session.save(&path).unwrap();

    // Delete the file
    fs::remove_file(&file_path).unwrap();

    // Load session - should succeed but log warning
    let result = Session::load(&path);
    assert!(result.is_ok());
    let loaded = result.unwrap();
    assert_eq!(loaded.groups.len(), 1);
    assert_eq!(loaded.groups[0].files[0].path, file_path);
}
