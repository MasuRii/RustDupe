use rustdupe::cache::{CacheEntry, CacheError, HashCache};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;
use tempfile::NamedTempFile;

#[test]
fn test_open_corrupted_database() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Write garbage to the file
    {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(b"not a sqlite database").unwrap();
    }

    // Attempt to open it - should fail
    let res = HashCache::new(path);
    assert!(res.is_err());
}

#[test]
fn test_recovery_on_corruption() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // 1. Create a corrupted file
    {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(b"corrupted garbage").unwrap();
    }

    // 2. Try to open it with "recovery" logic (to be implemented)
    // For now, let's just see if we can manually recover
    let res = HashCache::new(path);
    assert!(res.is_err());

    // Manual recovery logic simulation
    if res.is_err() {
        fs::remove_file(path).unwrap();
        let cache = HashCache::new(path).expect("Should succeed after deleting corrupted file");
        assert!(cache.clear().is_ok());
    }
}

#[test]
fn test_cache_on_readonly_filesystem() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // 1. Create a valid database first
    {
        let cache = HashCache::new(path).unwrap();
        let entry = CacheEntry {
            path: PathBuf::from("test"),
            size: 0,
            mtime: SystemTime::now(),
            inode: None,
            prehash: [0u8; 32],
            fullhash: None,
        };
        cache.insert_prehash(&entry, [0u8; 32]).unwrap();
        cache.close().unwrap();
    }

    // 2. Set it to read-only
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(path, perms.clone()).unwrap();

    // 3. Opening it might succeed if tables already exist, but writing SHOULD fail
    let cache_res = HashCache::new(path);

    if let Ok(cache) = cache_res {
        let entry = CacheEntry {
            path: PathBuf::from("test2"),
            size: 0,
            mtime: SystemTime::now(),
            inode: None,
            prehash: [0u8; 32],
            fullhash: None,
        };
        // This should fail because the database is read-only
        let res = cache.insert_prehash(&entry, [0u8; 32]);
        assert!(res.is_err(), "Insert should fail on read-only database");
    }

    // Cleanup: must set back to writeable to allow tempfile to delete it
    perms.set_readonly(false);
    let _ = fs::set_permissions(path, perms);
}
