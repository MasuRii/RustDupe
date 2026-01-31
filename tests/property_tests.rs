use proptest::prelude::*;
use rustdupe::duplicates::group_by_size;
use rustdupe::scanner::hasher::Hasher;
use rustdupe::scanner::FileEntry;
use std::fs;
use std::time::SystemTime;
use tempfile::TempDir;

proptest! {
    #[test]
    fn test_hash_determinism(content in "\\PC*") {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.bin");
        fs::write(&path, content.as_bytes()).unwrap();

        let hasher = Hasher::new();
        let hash1 = hasher.full_hash(&path).unwrap();
        let hash2 = hasher.full_hash(&path).unwrap();

        prop_assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_consistency_with_prehash(content in "\\PC*") {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.bin");
        fs::write(&path, content.as_bytes()).unwrap();

        let hasher = Hasher::new();
        let prehash = hasher.prehash(&path).unwrap();
        let full_hash = hasher.full_hash(&path).unwrap();

        if content.len() <= 4096 {
            prop_assert_eq!(prehash, full_hash);
        }
    }

    #[test]
    fn test_group_by_size_invariants(sizes in prop::collection::vec(0u64..1000, 0..50)) {
        let entries: Vec<FileEntry> = sizes.iter().enumerate().map(|(i, &size)| {
            FileEntry::new(
                std::path::PathBuf::from(format!("/fake/path/{}", i)),
                size,
                SystemTime::now()
            )
        }).collect();

        let (groups, stats) = group_by_size(entries.clone());

        // Invariant: All files in a group must have the same size
        for (size, files) in &groups {
            for file in files {
                prop_assert_eq!(file.size, *size);
            }
            // Invariant: Each group must have at least 2 files
            prop_assert!(files.len() >= 2);
        }

        // Invariant: total_files = input size
        prop_assert_eq!(stats.total_files, entries.len());

        // Invariant: potential_duplicates = sum of files in all groups
        let sum_files: usize = groups.values().map(|v| v.len()).sum();
        prop_assert_eq!(stats.potential_duplicates, sum_files);
    }

    #[test]
    fn test_hash_symmetry(content1 in "\\PC*", content2 in "\\PC*") {
        let dir = TempDir::new().unwrap();
        let path1 = dir.path().join("test1.bin");
        let path2 = dir.path().join("test2.bin");
        fs::write(&path1, content1.as_bytes()).unwrap();
        fs::write(&path2, content2.as_bytes()).unwrap();

        let hasher = Hasher::new();
        let hash1 = hasher.full_hash(&path1).unwrap();
        let hash2 = hasher.full_hash(&path2).unwrap();

        let equal12 = hash1 == hash2;
        let equal21 = hash2 == hash1;

        prop_assert_eq!(equal12, equal21);
    }
}
