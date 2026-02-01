//! I/O operations for scan sessions.

use crate::session::data::Session;
use anyhow::Result;
use std::path::Path;

impl Session {
    /// Saves the session to a file.
    pub fn save(&self, _path: &Path) -> Result<()> {
        todo!("Implement session save")
    }

    /// Loads a session from a file.
    pub fn load(_path: &Path) -> Result<Self> {
        todo!("Implement session load")
    }
}
