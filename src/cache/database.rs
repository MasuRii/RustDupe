//! SQLite-backed hash cache database.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// Persistent cache for file hashes using SQLite.
pub struct HashCache {
    #[allow(dead_code)]
    conn: Connection,
}

impl HashCache {
    /// Opens or creates a new hash cache at the specified path.
    pub fn new(_path: &Path) -> Result<Self> {
        // Implementation will be added in task 3.1.3
        Err(anyhow::anyhow!("Not implemented"))
    }
}
