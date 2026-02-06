use rustdupe::scanner::hasher::Hasher;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_mmap_hashing_matches_regular() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("large_file.bin");

    // Create a 1MB file (larger than some thresholds but small for testing)
    // We'll set the threshold to 512KB for this test
    let content = vec![0u8; 1024 * 1024];
    let mut file = File::create(&path).unwrap();
    file.write_all(&content).unwrap();

    let hasher_no_mmap = Hasher::new().with_mmap(false);
    let hasher_mmap = Hasher::new()
        .with_mmap(true)
        .with_mmap_threshold(512 * 1024);

    let hash1 = hasher_no_mmap.full_hash(&path).unwrap();
    let hash2 = hasher_mmap.full_hash(&path).unwrap();

    assert_eq!(
        hash1, hash2,
        "Mmap hashing should produce the same result as regular hashing"
    );
}

#[test]
fn test_mmap_hashing_fallback_on_missing_file() {
    let hasher = Hasher::new().with_mmap(true).with_mmap_threshold(0); // Always use mmap if enabled

    let path = std::path::Path::new("non_existent_file_12345.bin");
    let result = hasher.full_hash(path);

    assert!(result.is_err());
}

#[test]
fn test_mmap_hashing_below_threshold() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("small_file.bin");

    let content = b"small content";
    let mut file = File::create(&path).unwrap();
    file.write_all(content).unwrap();

    let hasher = Hasher::new()
        .with_mmap(true)
        .with_mmap_threshold(1024 * 1024); // 1MB threshold

    let hash = hasher.full_hash(&path).unwrap();
    assert_eq!(hash, *blake3::hash(content).as_bytes());
}
