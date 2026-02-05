use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::scanner::FileEntry;
use std::time::{Instant, SystemTime};
use tempfile::TempDir;

#[test]
fn test_bloom_filter_performance_reduction() {
    let temp_dir = TempDir::new().unwrap();

    // We want to verify that the multi-phase approach (Size -> Prehash -> Full)
    // reduces the total hashing work significantly compared to full hashing.
    // The Bloom filters further optimize this by allowing early rejection.

    let num_files = 1000;
    let num_dups = 50; // 50 pairs of duplicates
    let mut files = Vec::new();

    // Create many unique files
    for i in 0..num_files {
        let path = temp_dir.path().join(format!("unique_{}.txt", i));
        let size = 1000 + i as u64;
        let content = vec![i as u8; 10]; // Small content, unique size
        std::fs::write(&path, &content).unwrap();
        files.push(FileEntry::new(path, size, SystemTime::now()));
    }

    // Create some duplicate files
    for i in 0..num_dups {
        let size = 5000 + i as u64;
        let content = vec![(i % 256) as u8; 100];

        let path_a = temp_dir.path().join(format!("dup_{}_a.txt", i));
        std::fs::write(&path_a, &content).unwrap();
        let path_b = temp_dir.path().join(format!("dup_{}_b.txt", i));
        std::fs::write(&path_b, &content).unwrap();

        files.push(FileEntry::new(path_a, size, SystemTime::now()));
        files.push(FileEntry::new(path_b, size, SystemTime::now()));
    }

    let total_input_files = files.len();

    let start = Instant::now();
    let config = FinderConfig::default().with_bloom_fp_rate(0.01);
    let finder = DuplicateFinder::new(config);
    let (groups, summary) = finder.find_duplicates_from_files(files).unwrap();
    let duration = start.elapsed();

    println!("Scan of {} files took {:?}", total_input_files, duration);

    // Verify results
    assert_eq!(groups.len(), num_dups);

    // Verification of hash computation reduction:
    // Total files = 1000 + 100 = 1100
    // Files with unique sizes = 1000
    // Files with duplicate sizes = 100
    // Prehashes computed should be 100 (only for files that matched in size)
    // Full hashes computed should be 100 (since they all match in prehash too)
    // Total hash calls = 200

    // If we hashed everything (naive), we would have 1100 full hashes.
    // Reduction = (1100 - 200) / 1100 = 81.8%

    let total_hashes = (summary.total_files - summary.eliminated_by_size) // Prehashes
                     + (summary.total_files - summary.eliminated_by_size - summary.eliminated_by_prehash); // Full hashes

    let reduction = 1.0 - (total_hashes as f64 / summary.total_files as f64);
    println!("Hash computation reduction: {:.1}%", reduction * 100.0);

    assert!(
        reduction >= 0.3,
        "Hash reduction should be at least 30%, got {:.1}%",
        reduction * 100.0
    );
}

#[test]
fn test_bloom_filter_false_positive_rate() {
    let temp_dir = TempDir::new().unwrap();

    // Create a scenario where Bloom filters might have false positives
    // by using a very high number of unique items and a specific FP rate.

    let num_unique = 5000;
    let mut files = Vec::new();

    for i in 0..num_unique {
        let path = temp_dir.path().join(format!("file_{}.txt", i));
        // All have unique sizes
        let size = 10 + i as u64;
        std::fs::write(&path, "content").unwrap();
        files.push(FileEntry::new(path, size, SystemTime::now()));
    }

    // Set a controlled FP rate
    let fp_rate = 0.05; // 5%
    let config = FinderConfig::default().with_bloom_fp_rate(fp_rate);
    let finder = DuplicateFinder::new(config);
    let (_, summary) = finder.find_duplicates_from_files(files).unwrap();

    println!(
        "Bloom Size unique identified: {}",
        summary.bloom_size_unique
    );
    println!("Bloom Size false positives: {}", summary.bloom_size_fp);
    println!(
        "Bloom Size observed FP rate: {:.4}%",
        summary.bloom_size_fp_rate()
    );

    // The observed FP rate should be reasonably close to the configured rate.
    // Since it's probabilistic, we allow some margin.
    // Bloom filter FP rate is usually an UPPER bound.
    assert!(
        summary.bloom_size_fp_rate() / 100.0 <= fp_rate * 2.0,
        "Observed FP rate {:.4}% too much higher than configured {}%",
        summary.bloom_size_fp_rate(),
        fp_rate * 100.0
    );
}

#[test]
fn test_bloom_filter_memory_estimation() {
    // Verifies that Bloom filters use reasonable memory.
    // growable-bloom-filter is quite efficient.
    // 1M items at 1% FP rate should be around 1.2MB per filter (we use 2 per stage).
    // Total for 2 stages = 4 filters = ~5MB.

    // Since we don't have a direct way to measure memory of the struct,
    // we just verify that it can handle a large number of items without crashing.

    let num_items = 100_000;
    let fp_rate = 0.01;

    use growable_bloom_filter::GrowableBloom;
    let mut bloom = GrowableBloom::new(fp_rate, num_items);

    let start = Instant::now();
    for i in 0..num_items {
        bloom.insert(i);
    }
    let duration = start.elapsed();

    println!(
        "Inserting {} items into Bloom filter took {:?}",
        num_items, duration
    );
    assert!(duration.as_secs() < 1, "Bloom insertion too slow");

    for i in 0..num_items {
        assert!(bloom.contains(i));
    }
}
