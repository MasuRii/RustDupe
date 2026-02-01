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
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read session file: {}", path.display()))?;

        let envelope: SessionEnvelope = serde_json::from_str(&content).context(
            "Failed to parse session envelope. The file might be corrupted or in an old format.",
        )?;

        // Re-serialize the session to verify checksum
        // MUST use the same serialization settings as to_json (compact)
        let session_json = serde_json::to_string(&envelope.session)
            .context("Failed to re-serialize session for integrity check")?;

        let mut hasher = Sha256::new();
        hasher.update(session_json.as_bytes());
        let calculated_checksum = format!("{:x}", hasher.finalize());

        if calculated_checksum != envelope.checksum {
            anyhow::bail!("Session integrity check failed: checksum mismatch. The file may have been tampered with or corrupted.");
        }

        let session = envelope.session;

        // Validate version
        if session.version != crate::session::data::SESSION_VERSION {
            anyhow::bail!(
                "Unsupported session version: {}. Current version is {}.",
                session.version,
                crate::session::data::SESSION_VERSION
            );
        }

        // Validate that referenced files still exist
        for group in &session.groups {
            for file in &group.files {
                if !file.exists() {
                    log::warn!(
                        "File referenced in session no longer exists: {}",
                        file.display()
                    );
                }
            }
        }

        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::data::{SessionGroup, SessionSettings};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_session_to_json() {
        let settings = SessionSettings::default();
        let groups = vec![SessionGroup {
            id: 1,
            hash: [0u8; 32],
            size: 100,
            files: vec!["/tmp/a.txt".into(), "/tmp/b.txt".into()],
            reference_paths: Vec::new(),
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
            reference_paths: Vec::new(),
        }];
        let mut session = Session::new(vec!["/tmp".into()], settings, groups);
        session.user_selections.insert("/tmp/c.txt".into());

        session.save(&path).unwrap();

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"checksum\":"));
        assert!(content.contains("/tmp/c.txt"));
    }

    #[test]
    fn test_session_load_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let settings = SessionSettings::default();
        let groups = vec![SessionGroup {
            id: 1,
            hash: [1u8; 32],
            size: 200,
            files: vec!["/tmp/c.txt".into(), "/tmp/d.txt".into()],
            reference_paths: Vec::new(),
        }];
        let mut session = Session::new(vec!["/tmp".into()], settings, groups);
        session.user_selections.insert("/tmp/c.txt".into());

        session.save(&path).unwrap();

        let loaded = Session::load(&path).unwrap();
        assert_eq!(loaded.version, session.version);
        assert_eq!(loaded.scan_paths, session.scan_paths);
        assert_eq!(loaded.groups.len(), session.groups.len());
        assert_eq!(loaded.user_selections.len(), session.user_selections.len());
        assert!(loaded
            .user_selections
            .contains(&PathBuf::from("/tmp/c.txt")));
    }

    #[test]
    fn test_session_navigation_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session_nav.json");

        let mut session = Session::new(vec!["/tmp".into()], SessionSettings::default(), vec![]);
        session.group_index = 5;
        session.file_index = 2;

        session.save(&path).unwrap();

        let loaded = Session::load(&path).unwrap();
        assert_eq!(loaded.group_index, 5);
        assert_eq!(loaded.file_index, 2);
    }

    #[test]
    fn test_session_load_corrupted_checksum() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let settings = SessionSettings::default();
        let session = Session::new(vec!["/tmp".into()], settings, vec![]);
        session.save(&path).unwrap();

        // Corrupt the checksum
        let mut content = std::fs::read_to_string(&path).unwrap();
        content = content.replace("\"checksum\": \"", "\"checksum\": \"bad");
        std::fs::write(&path, content).unwrap();

        let result = Session::load(&path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("integrity check failed"));
    }

    #[test]
    fn test_session_load_invalid_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let settings = SessionSettings::default();
        let mut session = Session::new(vec!["/tmp".into()], settings, vec![]);
        session.version = 999; // Invalid version

        session.save(&path).unwrap();

        let result = Session::load(&path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported session version"));
    }
}
