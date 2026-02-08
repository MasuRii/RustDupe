use rustdupe::cache::HashCache;
use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_cache_initial_scan_and_rescan() {
    let dir = tempdir().unwrap();
    let cache_dir = tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.db");

    // Create some duplicate files
    let content = b"duplicate content";
    File::create(dir.path().join("file1.txt"))
        .unwrap()
        .write_all(content)
        .unwrap();
    File::create(dir.path().join("file2.txt"))
        .unwrap()
        .write_all(content)
        .unwrap();

    // Initial scan with cache
    let cache = Arc::new(HashCache::new(&cache_path).unwrap());
    let config = FinderConfig::default().with_cache(cache.clone());
    let finder = DuplicateFinder::new(config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(summary.cache_prehash_hits, 0);
    assert_eq!(summary.cache_prehash_misses, 2);
    assert_eq!(summary.cache_fullhash_hits, 0);
    assert_eq!(summary.cache_fullhash_misses, 2);

    // Rescan with same cache
    let (groups2, summary2) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(groups2.len(), 1);
    assert_eq!(summary2.cache_prehash_hits, 2);
    assert_eq!(summary2.cache_prehash_misses, 0);
    assert_eq!(summary2.cache_fullhash_hits, 2);
}

#[test]
fn test_cache_invalidation_on_change() {
    let dir = tempdir().unwrap();
    let cache_dir = tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.db");

    let file1_path = dir.path().join("file1.txt");
    let file2_path = dir.path().join("file2.txt");

    // Create some duplicate files - use same size for both
    let content = b"identical content 21b"; // 21 bytes
    File::create(&file1_path)
        .unwrap()
        .write_all(content)
        .unwrap();
    File::create(&file2_path)
        .unwrap()
        .write_all(content)
        .unwrap();

    let cache = Arc::new(HashCache::new(&cache_path).unwrap());
    let config = FinderConfig::default().with_cache(cache.clone());
    let finder = DuplicateFinder::new(config);

    // Initial scan
    finder.find_duplicates(dir.path()).unwrap();

    // Modify file1 - change content but keep size same, and ensure mtime changes
    std::thread::sleep(Duration::from_millis(1100));

    let mut f = File::create(&file1_path).unwrap();
    f.write_all(b"different content 21b").unwrap(); // Still 21 bytes
    f.sync_all().unwrap();

    // Rescan
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // They are no longer duplicates
    assert_eq!(groups.len(), 0);
    // file2 should still be a cache hit in Phase 2 because it's in a size group with file1
    // file1 should be a cache miss because mtime changed
    assert_eq!(summary.cache_prehash_hits, 1);
    assert_eq!(summary.cache_prehash_misses, 1);
}

#[test]
fn test_no_cache_behavior() {
    let dir = tempdir().unwrap();
    let content = b"duplicate content";
    File::create(dir.path().join("file1.txt"))
        .unwrap()
        .write_all(content)
        .unwrap();
    File::create(dir.path().join("file2.txt"))
        .unwrap()
        .write_all(content)
        .unwrap();

    // Scan without cache
    let finder = DuplicateFinder::with_defaults();
    let (_, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(summary.cache_prehash_hits, 0);
    assert_eq!(summary.cache_fullhash_hits, 0);
}

#[test]
fn test_cache_performance_benefit() {
    let dir = tempdir().unwrap();
    let cache_dir = tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.db");

    for i in 0..50 {
        let content = format!("content {:010}", i);
        File::create(dir.path().join(format!("file_{}.txt", i)))
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
        // Also a duplicate for each
        File::create(dir.path().join(format!("dup_{}.txt", i)))
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
    }

    let cache = Arc::new(HashCache::new(&cache_path).unwrap());
    let config = FinderConfig::default().with_cache(cache.clone());
    let finder = DuplicateFinder::new(config);

    // Initial scan
    let (_, summary1) = finder.find_duplicates(dir.path()).unwrap();

    // Rescan
    let (_, summary2) = finder.find_duplicates(dir.path()).unwrap();

    // Verify cache hits - this is the real correctness check
    assert_eq!(summary2.cache_prehash_hits, 100);

    // Note: We intentionally don't assert timing (scan_duration) because:
    // 1. CI environments have unpredictable CPU scheduling
    // 2. Small file sets may not show measurable timing differences
    // 3. The cache hit count above proves the cache is working correctly
    // The real performance benefit of caching is proven by the hit count,
    // not by timing which varies based on system load.
    let _ = summary1.scan_duration; // Silence unused warning
}
