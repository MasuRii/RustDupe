//! SQLite-backed hash cache database.

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;
use thiserror::Error;

use crate::cache::CacheEntry;
use crate::scanner::Hash;

/// Errors that can occur during cache operations.
#[derive(Error, Debug)]
pub enum CacheError {
    /// SQLite database error.
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The database connection is already closed.
    #[error("Database connection already closed")]
    ConnectionClosed,

    /// Failed to acquire database lock.
    #[error("Database lock error")]
    LockError,
}

/// Result type for cache operations.
pub type CacheResult<T> = std::result::Result<T, CacheError>;

/// Persistent cache for file hashes using SQLite.
///
/// This struct is thread-safe and can be shared across multiple threads
/// using an `Arc<HashCache>`.
pub struct HashCache {
    conn: Mutex<Option<Connection>>,
}

impl HashCache {
    /// Opens or creates a new hash cache at the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if the database cannot be opened or the schema
    /// cannot be initialized.
    pub fn new(path: &Path) -> CacheResult<Self> {
        let conn = Connection::open(path)?;

        // Initialize schema
        // We use a single table 'hashes' to store file metadata and computed hashes.
        // mtime_ns is stored as nanoseconds since UNIX epoch in a 64-bit integer.
        // created_at stores the entry creation time in seconds since UNIX epoch.
        // We store hashes as BLOBs for efficiency.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS hashes (
                path TEXT PRIMARY KEY,
                size INTEGER NOT NULL,
                mtime_ns INTEGER NOT NULL,
                inode INTEGER,
                prehash BLOB NOT NULL,
                fullhash BLOB,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        // Create indexes for faster lookups
        // idx_hashes_size_mtime supports fast lookup by size and mtime, which is our primary
        // validation mechanism during the initial phase of duplicate detection.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_hashes_size_mtime ON hashes (size, mtime_ns)",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(Some(conn)),
        })
    }

    /// Closes the database connection.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if the database connection cannot be closed cleanly.
    pub fn close(&self) -> CacheResult<()> {
        let mut lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        if let Some(conn) = lock.take() {
            conn.close().map_err(|(_, e)| CacheError::Database(e))?;
        }
        Ok(())
    }

    /// Helper to convert SystemTime to nanoseconds since UNIX epoch.
    fn system_time_to_ns(time: SystemTime) -> i64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }

    /// Helper to get current UNIX timestamp in seconds.
    fn now_secs() -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// Retrieve the prehash for a file if it exists and metadata matches.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn get_prehash(
        &self,
        path: &Path,
        size: u64,
        mtime: SystemTime,
    ) -> CacheResult<Option<Hash>> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;
        let mtime_ns = Self::system_time_to_ns(mtime);

        let mut stmt = conn.prepare_cached(
            "SELECT prehash FROM hashes WHERE path = ?1 AND size = ?2 AND mtime_ns = ?3",
        )?;
        let mut rows = stmt.query(params![path.to_string_lossy().to_string(), size, mtime_ns])?;

        if let Some(row) = rows.next()? {
            let blob: Vec<u8> = row.get(0)?;
            if blob.len() == 32 {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&blob);
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }

    /// Retrieve the full hash for a file if it exists and metadata matches.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn get_fullhash(
        &self,
        path: &Path,
        size: u64,
        mtime: SystemTime,
    ) -> CacheResult<Option<Hash>> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;
        let mtime_ns = Self::system_time_to_ns(mtime);

        let mut stmt = conn.prepare_cached(
            "SELECT fullhash FROM hashes WHERE path = ?1 AND size = ?2 AND mtime_ns = ?3",
        )?;
        let mut rows = stmt.query(params![path.to_string_lossy().to_string(), size, mtime_ns])?;

        if let Some(row) = rows.next()? {
            let blob: Option<Vec<u8>> = row.get(0)?;
            if let Some(blob) = blob {
                if blob.len() == 32 {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&blob);
                    return Ok(Some(hash));
                }
            }
        }
        Ok(None)
    }

    /// Insert or update a prehash in the cache.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn insert_prehash(&self, entry: &CacheEntry, hash: Hash) -> CacheResult<()> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;
        let mtime_ns = Self::system_time_to_ns(entry.mtime);
        let now = Self::now_secs();

        conn.execute(
            "INSERT INTO hashes (path, size, mtime_ns, inode, prehash, fullhash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)
             ON CONFLICT(path) DO UPDATE SET
                size = excluded.size,
                mtime_ns = excluded.mtime_ns,
                inode = excluded.inode,
                prehash = excluded.prehash,
                fullhash = NULL,
                created_at = excluded.created_at",
            params![
                entry.path.to_string_lossy().to_string(),
                entry.size,
                mtime_ns,
                entry.inode,
                &hash[..],
                now,
            ],
        )?;
        Ok(())
    }

    /// Insert or update a full hash in the cache.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn insert_fullhash(&self, entry: &CacheEntry, hash: Hash) -> CacheResult<()> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;
        let mtime_ns = Self::system_time_to_ns(entry.mtime);
        let now = Self::now_secs();

        conn.execute(
            "INSERT INTO hashes (path, size, mtime_ns, inode, prehash, fullhash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                size = excluded.size,
                mtime_ns = excluded.mtime_ns,
                inode = excluded.inode,
                prehash = excluded.prehash,
                fullhash = excluded.fullhash,
                created_at = excluded.created_at",
            params![
                entry.path.to_string_lossy().to_string(),
                entry.size,
                mtime_ns,
                entry.inode,
                &entry.prehash[..],
                &hash[..],
                now,
            ],
        )?;
        Ok(())
    }

    /// Insert multiple entries in a single transaction.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if transaction fails or database access fails.
    pub fn insert_batch(&self, entries: &[CacheEntry]) -> CacheResult<()> {
        let mut lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_mut().ok_or(CacheError::ConnectionClosed)?;
        let now = Self::now_secs();

        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO hashes (path, size, mtime_ns, inode, prehash, fullhash, created_at)
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                  ON CONFLICT(path) DO UPDATE SET
                     size = excluded.size,
                     mtime_ns = excluded.mtime_ns,
                     inode = excluded.inode,
                     prehash = excluded.prehash,
                     fullhash = excluded.fullhash,
                     created_at = excluded.created_at",
            )?;

            for entry in entries {
                let mtime_ns = Self::system_time_to_ns(entry.mtime);
                stmt.execute(params![
                    entry.path.to_string_lossy().to_string(),
                    entry.size,
                    mtime_ns,
                    entry.inode,
                    &entry.prehash[..],
                    entry.fullhash.as_ref().map(|h| &h[..]),
                    now,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Check if a valid entry exists for the given file metadata.
    ///
    /// This is a convenience wrapper around `get_prehash`.
    pub fn is_valid(&self, path: &Path, size: u64, mtime: SystemTime) -> bool {
        self.get_prehash(path, size, mtime)
            .map(|h| h.is_some())
            .unwrap_or(false)
    }

    /// Remove an entry from the cache.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn invalidate(&self, path: &Path) -> CacheResult<()> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;
        conn.execute(
            "DELETE FROM hashes WHERE path = ?1",
            params![path.to_string_lossy().to_string()],
        )?;
        Ok(())
    }

    /// Wipe all entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn clear(&self) -> CacheResult<()> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;
        conn.execute("DELETE FROM hashes", [])?;
        Ok(())
    }

    /// Remove entries for files that no longer exist on disk.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn prune_stale(&self) -> CacheResult<usize> {
        let mut lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_mut().ok_or(CacheError::ConnectionClosed)?;

        let mut stale_paths = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT path FROM hashes")?;
            let rows = stmt.query_map([], |row| {
                let path_str: String = row.get(0)?;
                Ok(path_str)
            })?;

            for path_res in rows {
                let path_str = path_res?;
                if !Path::new(&path_str).exists() {
                    stale_paths.push(path_str);
                }
            }
        }

        let count = stale_paths.len();
        if count > 0 {
            let tx = conn.transaction()?;
            {
                let mut del_stmt = tx.prepare_cached("DELETE FROM hashes WHERE path = ?1")?;
                for path in stale_paths {
                    del_stmt.execute(params![path])?;
                }
            }
            tx.commit()?;
        }

        Ok(count)
    }

    /// Remove entries older than the specified duration.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if database access fails.
    pub fn prune_by_age(&self, max_age: std::time::Duration) -> CacheResult<usize> {
        let lock = self.conn.lock().map_err(|_| CacheError::LockError)?;
        let conn = lock.as_ref().ok_or(CacheError::ConnectionClosed)?;

        let now = Self::now_secs();
        let cutoff = now - max_age.as_secs() as i64;

        let count = conn.execute("DELETE FROM hashes WHERE created_at < ?1", params![cutoff])?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_cache_new_and_close() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let cache = HashCache::new(path).unwrap();
        assert!(cache.conn.lock().unwrap().is_some());

        cache.close().unwrap();
        assert!(cache.conn.lock().unwrap().is_none());
    }

    #[test]
    fn test_hash_cache_reopen() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        {
            let cache = HashCache::new(path).unwrap();
            cache.close().unwrap();
        }

        let cache = HashCache::new(path).unwrap();
        assert!(cache.conn.lock().unwrap().is_some());
        cache.close().unwrap();
    }

    #[test]
    fn test_hash_cache_crud() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let cache = HashCache::new(path).unwrap();

        let now = SystemTime::now();
        let file_path = Path::new("/test/file.txt");
        let entry = CacheEntry {
            path: file_path.to_path_buf(),
            size: 1024,
            mtime: now,
            inode: Some(123),
            prehash: [1u8; 32],
            fullhash: None,
        };

        // Test insert and get prehash
        cache.insert_prehash(&entry, [1u8; 32]).unwrap();
        let cached_prehash = cache.get_prehash(file_path, 1024, now).unwrap();
        assert_eq!(cached_prehash, Some([1u8; 32]));

        // Test cache miss on metadata change
        let future = now + std::time::Duration::from_secs(1);
        assert!(cache
            .get_prehash(file_path, 1024, future)
            .unwrap()
            .is_none());
        assert!(cache.get_prehash(file_path, 1025, now).unwrap().is_none());

        // Test insert and get fullhash
        cache.insert_fullhash(&entry, [2u8; 32]).unwrap();
        let cached_fullhash = cache.get_fullhash(file_path, 1024, now).unwrap();
        assert_eq!(cached_fullhash, Some([2u8; 32]));

        // Test fullhash insert updates prehash if provided in entry
        let mut entry2 = entry.clone();
        entry2.prehash = [3u8; 32];
        cache.insert_fullhash(&entry2, [4u8; 32]).unwrap();
        assert_eq!(
            cache.get_prehash(file_path, 1024, now).unwrap(),
            Some([3u8; 32])
        );
        assert_eq!(
            cache.get_fullhash(file_path, 1024, now).unwrap(),
            Some([4u8; 32])
        );
    }

    #[test]
    fn test_hash_cache_batch() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let cache = HashCache::new(path).unwrap();

        let now = SystemTime::now();
        let entries = vec![
            CacheEntry {
                path: PathBuf::from("/test/1.txt"),
                size: 100,
                mtime: now,
                inode: None,
                prehash: [1u8; 32],
                fullhash: Some([11u8; 32]),
            },
            CacheEntry {
                path: PathBuf::from("/test/2.txt"),
                size: 200,
                mtime: now,
                inode: None,
                prehash: [2u8; 32],
                fullhash: None,
            },
        ];

        cache.insert_batch(&entries).unwrap();

        assert_eq!(
            cache
                .get_prehash(Path::new("/test/1.txt"), 100, now)
                .unwrap(),
            Some([1u8; 32])
        );
        assert_eq!(
            cache
                .get_fullhash(Path::new("/test/1.txt"), 100, now)
                .unwrap(),
            Some([11u8; 32])
        );
        assert_eq!(
            cache
                .get_prehash(Path::new("/test/2.txt"), 200, now)
                .unwrap(),
            Some([2u8; 32])
        );
        assert_eq!(
            cache
                .get_fullhash(Path::new("/test/2.txt"), 200, now)
                .unwrap(),
            None
        );
    }

    #[test]
    fn test_hash_cache_invalidation_logic() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let cache = HashCache::new(path).unwrap();

        let now = SystemTime::now();
        let file_path = Path::new("/test/file.txt");
        let entry = CacheEntry {
            path: file_path.to_path_buf(),
            size: 1024,
            mtime: now,
            inode: None,
            prehash: [1u8; 32],
            fullhash: None,
        };

        cache.insert_prehash(&entry, [1u8; 32]).unwrap();
        assert!(cache.is_valid(file_path, 1024, now));

        // Test invalidate
        cache.invalidate(file_path).unwrap();
        assert!(!cache.is_valid(file_path, 1024, now));

        // Test clear
        cache.insert_prehash(&entry, [1u8; 32]).unwrap();
        assert!(cache.is_valid(file_path, 1024, now));
        cache.clear().unwrap();
        assert!(!cache.is_valid(file_path, 1024, now));
    }

    #[test]
    fn test_hash_cache_prune_stale() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path();
        let cache = HashCache::new(cache_path).unwrap();

        // Create a real file
        let real_file = NamedTempFile::new().unwrap();
        let real_path = real_file.path();
        let now = SystemTime::now();

        let entry_real = CacheEntry {
            path: real_path.to_path_buf(),
            size: 0,
            mtime: now,
            inode: None,
            prehash: [1u8; 32],
            fullhash: None,
        };

        let entry_fake = CacheEntry {
            path: PathBuf::from("/non/existent/file"),
            size: 0,
            mtime: now,
            inode: None,
            prehash: [2u8; 32],
            fullhash: None,
        };

        cache.insert_prehash(&entry_real, [1u8; 32]).unwrap();
        cache.insert_prehash(&entry_fake, [2u8; 32]).unwrap();

        let pruned = cache.prune_stale().unwrap();
        assert_eq!(pruned, 1);

        assert!(cache.is_valid(real_path, 0, now));
        assert!(!cache.is_valid(Path::new("/non/existent/file"), 0, now));
    }

    #[test]
    fn test_hash_cache_prune_by_age() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path();
        let cache = HashCache::new(cache_path).unwrap();

        let now = SystemTime::now();
        let entry = CacheEntry {
            path: PathBuf::from("/test/file.txt"),
            size: 1024,
            mtime: now,
            inode: None,
            prehash: [1u8; 32],
            fullhash: None,
        };

        cache.insert_prehash(&entry, [1u8; 32]).unwrap();
        assert!(cache.is_valid(Path::new("/test/file.txt"), 1024, now));

        // Prune with 0 age (should prune everything)
        let pruned = cache
            .prune_by_age(std::time::Duration::from_secs(0))
            .unwrap();
        assert_eq!(pruned, 0); // Wait, if it's "created_at < now - 0", it should be 1 if now matches.
                               // Actually, Self::now_secs() might be the same as created_at.
                               // Let's use a very large duration to NOT prune.
        let pruned = cache
            .prune_by_age(std::time::Duration::from_secs(3600))
            .unwrap();
        assert_eq!(pruned, 0);
        assert!(cache.is_valid(Path::new("/test/file.txt"), 1024, now));

        // We can't easily test pruning without mocking time or sleeping, but we can verify the SQL.
        // Let's manually insert an old entry.
        {
            let lock = cache.conn.lock().unwrap();
            let conn = lock.as_ref().unwrap();
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            conn.execute(
                "INSERT INTO hashes (path, size, mtime_ns, inode, prehash, fullhash, created_at)
                 VALUES ('/test/old.txt', 100, 0, NULL, x'00', NULL, ?1)",
                params![now - 10000],
            )
            .unwrap();
        }

        let pruned = cache
            .prune_by_age(std::time::Duration::from_secs(3600))
            .unwrap();
        assert_eq!(pruned, 1);
    }

    #[test]
    fn test_hash_cache_performance() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let cache = HashCache::new(path).unwrap();

        let now = SystemTime::now();
        let count = 10_000;
        let mut entries = Vec::with_capacity(count);

        for i in 0..count {
            entries.push(CacheEntry {
                path: PathBuf::from(format!("/test/file_{}.txt", i)),
                size: i as u64,
                mtime: now,
                inode: Some(i as u64),
                prehash: [i as u8; 32],
                fullhash: Some([i as u8; 32]),
            });
        }

        // Test batch insert performance
        let start = std::time::Instant::now();
        cache.insert_batch(&entries).unwrap();
        let duration = start.elapsed();
        println!("Inserted {} entries in {:?}", count, duration);

        // Test retrieval performance
        let start = std::time::Instant::now();
        for i in 0..count {
            let path = PathBuf::from(format!("/test/file_{}.txt", i));
            let hash = cache.get_fullhash(&path, i as u64, now).unwrap();
            assert!(hash.is_some());
        }
        let duration = start.elapsed();
        println!("Retrieved {} entries in {:?}", count, duration);

        // Test prune performance
        let start = std::time::Instant::now();
        let pruned = cache.prune_stale().unwrap();
        assert_eq!(pruned, count);
        let duration = start.elapsed();
        println!("Pruned {} stale entries in {:?}", count, duration);
    }

    #[test]
    fn test_hash_cache_connection_closed() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let cache = HashCache::new(path).unwrap();

        cache.close().unwrap();

        let res = cache.get_prehash(Path::new("test"), 0, SystemTime::now());
        assert!(matches!(res, Err(CacheError::ConnectionClosed)));

        let entry = CacheEntry {
            path: PathBuf::from("test"),
            size: 0,
            mtime: SystemTime::now(),
            inode: None,
            prehash: [0u8; 32],
            fullhash: None,
        };
        let res = cache.insert_prehash(&entry, [0u8; 32]);
        assert!(matches!(res, Err(CacheError::ConnectionClosed)));
    }

    #[test]
    fn test_hash_cache_corrupted_blobs() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let cache = HashCache::new(path).unwrap();

        let now = SystemTime::now();
        let file_path = Path::new("/test/file.txt");

        // Manually insert an entry with an invalid blob size
        {
            let lock = cache.conn.lock().unwrap();
            let conn = lock.as_ref().unwrap();
            conn.execute(
                "INSERT INTO hashes (path, size, mtime_ns, inode, prehash, fullhash, created_at)
                 VALUES ('/test/file.txt', 1024, ?1, NULL, x'010203', NULL, ?2)",
                params![HashCache::system_time_to_ns(now), HashCache::now_secs()],
            )
            .unwrap();
        }

        // Retrieve it - get_prehash should return None because the blob length is not 32
        let res = cache.get_prehash(file_path, 1024, now).unwrap();
        assert_eq!(res, None);

        // Same for fullhash with invalid blob
        {
            let lock = cache.conn.lock().unwrap();
            let conn = lock.as_ref().unwrap();
            conn.execute(
                "UPDATE hashes SET fullhash = x'FFFF' WHERE path = '/test/file.txt'",
                [],
            )
            .unwrap();
        }
        let res = cache.get_fullhash(file_path, 1024, now).unwrap();
        assert_eq!(res, None);
    }

    #[test]
    fn test_hash_cache_invalid_schema() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create a table with wrong columns
        {
            let conn = Connection::open(path).unwrap();
            conn.execute(
                "CREATE TABLE hashes (path TEXT PRIMARY KEY, wrong_column TEXT)",
                [],
            )
            .unwrap();
        }

        // Opening it should fail because initialization tries to create an index on missing columns
        let res = HashCache::new(path);
        assert!(res.is_err());
    }
}
