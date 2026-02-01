//! Data structures for scan sessions.

use serde::{Deserialize, Serialize};

/// Represents a saved scan session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session format version.
    pub version: String,
}
