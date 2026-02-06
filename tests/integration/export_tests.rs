use rustdupe::duplicates::{DuplicateGroup, ScanSummary};
use rustdupe::scanner::FileEntry;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::SystemTime;

#[test]
fn test_filter_selected_logic() {
    let now = SystemTime::now();
    let file1 = PathBuf::from("/test/file1.txt");
    let file2 = PathBuf::from("/test/file2.txt");
    let file3 = PathBuf::from("/test/file3.txt");

    let groups = vec![
        DuplicateGroup::new(
            [1u8; 32],
            100,
            vec![
                FileEntry::new(file1.clone(), 100, now),
                FileEntry::new(file2.clone(), 100, now),
            ],
            Vec::new(),
        ),
        DuplicateGroup::new(
            [2u8; 32],
            200,
            vec![FileEntry::new(file3.clone(), 200, now)],
            Vec::new(),
        ),
    ];

    let summary = ScanSummary {
        total_files: 3,
        total_size: 400,
        duplicate_groups: 1,
        duplicate_files: 1,
        reclaimable_space: 100,
        ..Default::default()
    };

    let mut selections = BTreeSet::new();
    selections.insert(file2.clone());

    let (f_groups, f_summary) =
        rustdupe::duplicates::groups::filter_selected(&groups, &summary, &selections);

    // Only group 1 should be present, and only file 2 in it
    assert_eq!(f_groups.len(), 1);
    assert_eq!(f_groups[0].files.len(), 1);
    assert_eq!(f_groups[0].files[0].path, file2);

    // Summary should reflect only selected files
    assert_eq!(f_summary.duplicate_groups, 1);
    assert_eq!(f_summary.duplicate_files, 1);
    assert_eq!(f_summary.reclaimable_space, 100);
}
