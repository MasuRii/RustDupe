use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::scanner::FileEntry;
use std::time::SystemTime;

#[test]
fn test_bloom_filter_filtering_effectiveness() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create a large number of file entries with many unique sizes and some duplicates
    let num_unique = 100;
    let num_duplicates = 10;
    let mut files = Vec::new();

    // Unique sizes
    for i in 0..num_unique {
        let path = temp_dir.path().join(format!("unique_{}.txt", i));
        let size = 1000 + i as u64;
        let content = vec![i as u8; size as usize];
        std::fs::write(&path, content).unwrap();

        files.push(FileEntry::new(path, size, SystemTime::now()));
    }

    // Duplicate sizes (each size appears twice)
    for i in 0..num_duplicates {
        let size = 5000 + i as u64;
        let content = vec![(i + 100) as u8; size as usize];

        let path_a = temp_dir.path().join(format!("dup_{}_a.txt", i));
        std::fs::write(&path_a, &content).unwrap();

        let path_b = temp_dir.path().join(format!("dup_{}_b.txt", i));
        std::fs::write(&path_b, &content).unwrap();

        files.push(FileEntry::new(path_a, size, SystemTime::now()));
        files.push(FileEntry::new(path_b, size, SystemTime::now()));
    }

    let config = FinderConfig::default().with_bloom_fp_rate(0.01);
    let finder = DuplicateFinder::new(config);

    // find_duplicates_from_files uses the Bloom filter optimization
    let (groups, summary) = finder.find_duplicates_from_files(files).unwrap();

    // Verify results are correct
    assert_eq!(groups.len(), num_duplicates);
    assert_eq!(summary.total_files, num_unique + 2 * num_duplicates);

    // summary.eliminated_by_size should include the unique files filtered by Bloom
    // or by group_by_size later.
    assert!(summary.eliminated_by_size >= num_unique);
}

#[test]
fn test_bloom_filter_fp_rate_configuration() {
    let config = FinderConfig::default().with_bloom_fp_rate(0.001);
    assert_eq!(config.bloom_fp_rate, 0.001);

    let config = FinderConfig::default().with_bloom_fp_rate(0.2); // Should be clamped
    assert!(config.bloom_fp_rate <= 0.1);
}
