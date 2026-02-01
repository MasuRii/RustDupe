//! I/O operations for scan sessions.

use crate::session::data::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Envelope for session files to include integrity checks.
#[derive(Debug, Serialize, Deserialize)]
struct SessionEnvelope {
    /// SHA256 checksum of the serialized session data.
    checksum: String,
    /// The actual session data.
    session: Session,
}

impl Session {
    /// Saves the session to a file with an integrity checksum.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = self.to_json()?;
        let mut file = File::create(path)
            .with_context(|| format!("Failed to create session file: {}", path.display()))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("Failed to write session to: {}", path.display()))?;
        Ok(())
    }

    /// Serializes the session to a JSON string with an integrity checksum.
    pub fn to_json(&self) -> Result<String> {
        // First serialize the session to get the data to hash
        let session_json = serde_json::to_string(&self)
            .context("Failed to serialize session for checksum calculation")?;

        // Calculate SHA256 checksum
        let mut hasher = Sha256::new();
        hasher.update(session_json.as_bytes());
        let checksum = format!("{:x}", hasher.finalize());

        // Create the envelope
        let envelope = SessionEnvelope {
            checksum,
            session: self.clone(),
        };

        // Serialize the envelope with pretty printing for readability
        let final_json = serde_json::to_string_pretty(&envelope)
            .context("Failed to serialize session envelope")?;

        Ok(final_json)
    }

    /// Loads a session from a file and verifies its integrity.
    pub fn load(_path: &Path) -> Result<Self> {
        todo!("Implement session load")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::data::{SessionGroup, SessionSettings};
    use tempfile::tempdir;

    #[test]
    fn test_session_to_json() {
        let settings = SessionSettings::default();
        let groups = vec![SessionGroup {
            id: 1,
            hash: [0u8; 32],
            size: 100,
            files: vec!["/tmp/a.txt".into(), "/tmp/b.txt".into()],
        }];
        let session = Session::new(vec!["/tmp".into()], settings, groups);

        let json = session.to_json().unwrap();
        assert!(json.contains("\"checksum\":"));
        assert!(json.contains("\"session\":"));
        assert!(json.contains("\"version\":"));
    }

    #[test]
    fn test_session_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let settings = SessionSettings::default();
        let groups = vec![SessionGroup {
            id: 1,
            hash: [1u8; 32],
            size: 200,
            files: vec!["/tmp/c.txt".into(), "/tmp/d.txt".into()],
        }];
        let mut session = Session::new(vec!["/tmp".into()], settings, groups);
        session.user_selections.insert("/tmp/c.txt".into());

        session.save(&path).unwrap();

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"checksum\":"));
        assert!(content.contains("/tmp/c.txt"));
    }
}
