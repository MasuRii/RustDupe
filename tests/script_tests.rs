use rustdupe::duplicates::{DuplicateGroup, ScanSummary};
use rustdupe::output::script::{ScriptOutput, ScriptType};
use rustdupe::scanner::FileEntry;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[test]
fn test_script_output_integration() {
    let now = SystemTime::now();
    let groups = vec![
        DuplicateGroup::new(
            [1u8; 32],
            100,
            vec![
                FileEntry::new(PathBuf::from("/data/orig.txt"), 100, now),
                FileEntry::new(PathBuf::from("/data/dup1.txt"), 100, now),
                FileEntry::new(PathBuf::from("/data/dup2.txt"), 100, now),
            ],
            Vec::new(),
        ),
        DuplicateGroup::new(
            [2u8; 32],
            500,
            vec![
                FileEntry::new(PathBuf::from("/other/original.dat"), 500, now),
                FileEntry::new(PathBuf::from("/other/copy.dat"), 500, now),
            ],
            Vec::new(),
        ),
    ];

    let summary = ScanSummary {
        total_files: 5,
        total_size: 1300,
        duplicate_groups: 2,
        duplicate_files: 3,
        reclaimable_space: 700, // (100*2) + 500
        scan_duration: Duration::from_secs(2),
        ..Default::default()
    };

    // Test POSIX
    let output = ScriptOutput::new(&groups, &summary, ScriptType::Posix);
    let mut buffer = Vec::new();
    output.write_to(&mut buffer).unwrap();
    let script = String::from_utf8(buffer).unwrap();

    assert!(script.contains("# RustDupe Duplicate Deletion Script"));
    assert!(script.contains("Total duplicates found: 3"));
    assert!(script.contains("Reclaimable space: 700 B"));
    assert!(script.contains("# DELETE: '/data/dup1.txt'"));
    assert!(script.contains("# DELETE: '/data/dup2.txt'"));
    assert!(script.contains("# DELETE: '/other/copy.dat'"));
    assert!(script.contains("rm '/data/dup1.txt'"));
    assert!(script.contains("rm '/data/dup2.txt'"));
    assert!(script.contains("rm '/other/copy.dat'"));

    // Test PowerShell
    let output = ScriptOutput::new(&groups, &summary, ScriptType::PowerShell);
    let mut buffer = Vec::new();
    output.write_to(&mut buffer).unwrap();
    let script = String::from_utf8(buffer).unwrap();

    assert!(script.contains("Total duplicates found: 3"));
    assert!(script.contains("Reclaimable space: 700 B"));
    assert!(script.contains("# DELETE: '/data/dup1.txt'"));
    assert!(script.contains("Remove-Item -Path '/data/dup1.txt'"));
}

#[test]
fn test_script_with_special_paths() {
    let now = SystemTime::now();
    let groups = vec![DuplicateGroup::new(
        [3u8; 32],
        42,
        vec![
            FileEntry::new(PathBuf::from("/data/original.txt"), 42, now),
            FileEntry::new(PathBuf::from("/data/path with spaces.txt"), 42, now),
            FileEntry::new(PathBuf::from("/data/path'with'quotes.txt"), 42, now),
            FileEntry::new(PathBuf::from("/data/path$with$vars.txt"), 42, now),
        ],
        Vec::new(),
    )];

    let summary = ScanSummary {
        duplicate_files: 3,
        reclaimable_space: 126,
        ..Default::default()
    };

    // POSIX
    let output = ScriptOutput::new(&groups, &summary, ScriptType::Posix);
    let mut buffer = Vec::new();
    output.write_to(&mut buffer).unwrap();
    let script = String::from_utf8(buffer).unwrap();

    assert!(script.contains("'/data/path with spaces.txt'"));
    assert!(script.contains("'/data/path'\\''with'\\''quotes.txt'"));
    assert!(script.contains("'/data/path$with$vars.txt'"));

    // PowerShell
    let output = ScriptOutput::new(&groups, &summary, ScriptType::PowerShell);
    let mut buffer = Vec::new();
    output.write_to(&mut buffer).unwrap();
    let script = String::from_utf8(buffer).unwrap();

    assert!(script.contains("'/data/path with spaces.txt'"));
    assert!(script.contains("'/data/path''with''quotes.txt'"));
    assert!(script.contains("'/data/path$with$vars.txt'"));
}

#[test]
fn test_script_user_selections() {
    let now = SystemTime::now();
    let p1 = PathBuf::from("/data/1.txt");
    let p2 = PathBuf::from("/data/2.txt");
    let groups = vec![DuplicateGroup::new(
        [4u8; 32],
        10,
        vec![
            FileEntry::new(p1.clone(), 10, now),
            FileEntry::new(p2.clone(), 10, now),
        ],
        Vec::new(),
    )];

    let summary = ScanSummary {
        duplicate_files: 1,
        reclaimable_space: 10,
        ..Default::default()
    };

    // Select the first one, keep the second one
    let mut selections = BTreeSet::new();
    selections.insert(p1.clone());

    let output =
        ScriptOutput::new(&groups, &summary, ScriptType::Posix).with_user_selections(&selections);
    let mut buffer = Vec::new();
    output.write_to(&mut buffer).unwrap();
    let script = String::from_utf8(buffer).unwrap();

    assert!(script.contains("# DELETE: '/data/1.txt'"));
    assert!(script.contains("# KEEP:   '/data/2.txt'"));
    assert!(script.contains("rm '/data/1.txt'"));
    assert!(!script.contains("rm '/data/2.txt'"));
}
