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

    // Verify Bloom filter metrics
    assert!(summary.bloom_size_unique > 0);
    assert!(summary.bloom_size_unique <= num_unique);
    // FP rate should be reasonably low (with default 0.01 and 100 items, should be small)
    assert!(summary.bloom_size_fp_rate() < 10.0);
}

#[test]
fn test_bloom_filter_prehash_metrics() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create files with same size but different content
    let size = 1024;
    let num_unique_prehashes = 20;
    let mut files = Vec::new();

    for i in 0..num_unique_prehashes {
        let path = temp_dir.path().join(format!("unique_prehash_{}.txt", i));
        // First 4KB different
        let mut content = vec![0u8; size];
        content[0] = i as u8;
        std::fs::write(&path, content).unwrap();

        files.push(FileEntry::new(path, size as u64, SystemTime::now()));
    }

    // Add some actual duplicates
    let dup_size = 2048;
    let num_dups = 5;
    for i in 0..num_dups {
        let content = vec![i as u8; dup_size];
        let path_a = temp_dir.path().join(format!("dup_{}_a.txt", i));
        let path_b = temp_dir.path().join(format!("dup_{}_b.txt", i));
        std::fs::write(&path_a, &content).unwrap();
        std::fs::write(&path_b, &content).unwrap();

        files.push(FileEntry::new(path_a, dup_size as u64, SystemTime::now()));
        files.push(FileEntry::new(path_b, dup_size as u64, SystemTime::now()));
    }

    let config = FinderConfig::default().with_bloom_fp_rate(0.01);
    let finder = DuplicateFinder::new(config);

    let (_, summary) = finder.find_duplicates_from_files(files).unwrap();

    assert!(summary.bloom_prehash_unique > 0);
    assert!(summary.bloom_prehash_unique <= num_unique_prehashes);
    assert!(summary.bloom_prehash_fp_rate() < 10.0);
}

#[test]
fn test_bloom_filter_fp_rate_configuration() {
    let config = FinderConfig::default().with_bloom_fp_rate(0.001);
    assert_eq!(config.bloom_fp_rate, 0.001);

    let config = FinderConfig::default().with_bloom_fp_rate(0.2); // Should be clamped
    assert!(config.bloom_fp_rate <= 0.1);
}
