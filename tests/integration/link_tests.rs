use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::scanner::WalkerConfig;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_hardlinks_to_same_file_not_counted_as_duplicates() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.txt");
    let hardlink = dir.path().join("hardlink.txt");

    // Create original file
    File::create(&original)
        .unwrap()
        .write_all(b"identical content")
        .unwrap();

    // Create hardlink
    // This is platform-dependent but std::fs::hard_link works on both Unix and Windows (NTFS)
    if let Err(e) = fs::hard_link(&original, &hardlink) {
        eprintln!("Skipping hardlink test: failed to create hardlink: {}", e);
        return;
    }

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // Hardlinks should be seen as the same file (same inode/file index),
    // so one of them should be skipped by the walker IF supported.
    if rustdupe::scanner::hardlink::HardlinkTracker::is_supported() {
        assert!(
            groups.is_empty(),
            "Hardlinks should not be counted as duplicates on supported platforms"
        );
        assert_eq!(summary.total_files, 1);
    } else {
        // On unsupported platforms (like current Windows implementation),
        // they are seen as two files with identical content.
        assert_eq!(groups.len(), 1);
        assert_eq!(summary.total_files, 2);
    }
    assert_eq!(
        summary.duplicate_groups,
        if rustdupe::scanner::hardlink::HardlinkTracker::is_supported() {
            0
        } else {
            1
        }
    );
}

#[test]
fn test_symlinks_not_followed_by_default() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.txt");
    let symlink = dir.path().join("symlink.txt");

    File::create(&original)
        .unwrap()
        .write_all(b"content")
        .unwrap();

    // Create symlink
    let symlink_res = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&original, &symlink)
        }
        #[cfg(windows)]
        {
            // symlink_file requires admin rights or developer mode on some Windows versions
            std::os::windows::fs::symlink_file(&original, &symlink)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unsupported platform",
            ))
        }
    };

    if let Err(e) = symlink_res {
        eprintln!("Skipping symlink test: failed to create symlink: {}", e);
        return;
    }

    let finder = DuplicateFinder::with_defaults();
    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // By default, symlinks are not followed, so only the original file should be seen.
    assert_eq!(summary.total_files, 1);
    assert!(groups.is_empty());
}

#[test]
fn test_symlinks_followed_when_enabled() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.txt");
    let symlink = dir.path().join("symlink.txt");

    File::create(&original)
        .unwrap()
        .write_all(b"content")
        .unwrap();

    // Create symlink
    let symlink_res = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&original, &symlink)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(&original, &symlink)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unsupported platform",
            ))
        }
    };

    if let Err(e) = symlink_res {
        eprintln!("Skipping symlink test: failed to create symlink: {}", e);
        return;
    }

    let walker_config = WalkerConfig::default().with_follow_symlinks(true);
    let finder_config = FinderConfig::default().with_walker_config(walker_config);
    let finder = DuplicateFinder::new(finder_config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // If followed, the symlink points to the same file (same inode).
    // The HardlinkTracker should still catch it if it's the same inode and supported.
    if rustdupe::scanner::hardlink::HardlinkTracker::is_supported() {
        assert_eq!(summary.total_files, 1);
        assert!(groups.is_empty());
    } else {
        // On unsupported platforms, it sees both the original and the symlink-as-file
        assert_eq!(summary.total_files, 2);
        assert_eq!(groups.len(), 1);
    }
}

#[test]
fn test_symlink_cycle_detection() {
    // Symlink cycles are easier to test on Unix
    #[cfg(unix)]
    {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let link = sub.join("link");
        // Create a cycle: sub/link -> sub
        if let Err(e) = std::os::unix::fs::symlink(&sub, &link) {
            eprintln!("Skipping cycle test: failed to create symlink: {}", e);
            return;
        }

        let walker_config = WalkerConfig::default().with_follow_symlinks(true);
        let finder_config = FinderConfig::default().with_walker_config(walker_config);
        let finder = DuplicateFinder::new(finder_config);

        // This should not hang or panic
        let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

        // It should just see the directory (which is ignored by DuplicateFinder)
        // and avoid the infinite recursion.
        assert!(groups.is_empty());
        assert_eq!(summary.total_files, 0);
    }
}
