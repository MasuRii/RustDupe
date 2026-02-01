//! Application configuration management.
//!
//! This module handles loading and saving application-wide configuration
//! settings, such as the preferred TUI theme.

use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::cli::ThemeArg;

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Preferred TUI theme.
    #[serde(default)]
    pub theme: ThemeArg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeArg::Auto,
        }
    }
}

impl Config {
    /// Load the configuration from the default platform-specific path.
    pub fn load() -> Self {
        match Self::load_internal() {
            Ok(config) => config,
            Err(e) => {
                log::debug!("Failed to load config, using defaults: {}", e);
                Self::default()
            }
        }
    }

    fn load_internal() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save the configuration to the default platform-specific path.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default platform-specific configuration path.
    fn config_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "rustdupe", "rustdupe")
            .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;
        Ok(project_dirs.config_dir().join("config.json"))
    }
}

// Implement Default manually for ThemeArg in cli.rs if not already there,
// or use serde default attribute.
// ThemeArg already has #[derive(Default)]? No, let's check.
// I'll add Default to ThemeArg in cli.rs.
