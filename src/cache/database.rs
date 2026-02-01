//! SQLite-backed hash cache database.

use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

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
}

/// Result type for cache operations.
pub type CacheResult<T> = std::result::Result<T, CacheError>;

/// Persistent cache for file hashes using SQLite.
pub struct HashCache {
    conn: Option<Connection>,
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
        // We store hashes as BLOBs for efficiency.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS hashes (
                path TEXT PRIMARY KEY,
                size INTEGER NOT NULL,
                mtime_ns INTEGER NOT NULL,
                inode INTEGER,
                prehash BLOB NOT NULL,
                fullhash BLOB
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

        Ok(Self { conn: Some(conn) })
    }

    /// Closes the database connection.
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if the database connection cannot be closed cleanly.
    pub fn close(&mut self) -> CacheResult<()> {
        if let Some(conn) = self.conn.take() {
            conn.close().map_err(|(_, e)| CacheError::Database(e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_cache_new_and_close() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut cache = HashCache::new(path).unwrap();
        assert!(cache.conn.is_some());

        cache.close().unwrap();
        assert!(cache.conn.is_none());
    }

    #[test]
    fn test_hash_cache_reopen() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        {
            let mut cache = HashCache::new(path).unwrap();
            cache.close().unwrap();
        }

        let mut cache = HashCache::new(path).unwrap();
        assert!(cache.conn.is_some());
        cache.close().unwrap();
    }
}
