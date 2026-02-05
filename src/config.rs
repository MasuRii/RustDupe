//! Application configuration management.
//!
//! This module handles loading and saving application-wide configuration
//! settings, such as the preferred TUI theme and keybinding profile.
//!
//! # Custom Keybindings
//!
//! Custom keybindings can be defined in the config file using the `custom_keybindings`
//! section. Each entry maps an action name to a list of key specifications:
//!
//! ```json
//! {
//!     "custom_keybindings": {
//!         "navigate_down": ["n", "Ctrl+n"],
//!         "quit": ["x", "Ctrl+q"]
//!     }
//! }
//! ```
//!
//! ## Action Names
//!
//! - `navigate_up`, `navigate_down` - Move up/down in lists
//! - `next_group`, `previous_group` - Navigate between duplicate groups
//! - `go_to_top`, `go_to_bottom` - Jump to first/last item
//! - `toggle_select` - Toggle selection of current item
//! - `select_all_in_group`, `select_all_duplicates` - Bulk selection
//! - `select_oldest`, `select_newest`, `select_smallest`, `select_largest`
//! - `deselect_all` - Clear all selections
//! - `preview` - Preview file content
//! - `select_folder` - Enter folder selection mode
//! - `delete` - Delete selected files
//! - `toggle_theme` - Switch theme
//! - `confirm`, `cancel` - Confirm/cancel actions
//! - `quit` - Exit application
//!
//! ## Key Specifications
//!
//! - Simple keys: `j`, `k`, `Space`, `Enter`, `Esc`
//! - Arrow keys: `Up`, `Down`, `Left`, `Right`
//! - Special keys: `PageUp`, `PageDown`, `Home`, `End`, `Delete`
//! - Function keys: `F1`, `F2`, ..., `F12`
//! - With modifiers: `Ctrl+c`, `Alt+j`, `Shift+Enter`, `Ctrl+Shift+a`

use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::cli::ThemeArg;
use crate::tui::keybindings::KeybindingProfile;

/// Type alias for custom keybinding overrides.
///
/// Maps action names (e.g., "navigate_down") to lists of key specifications
/// (e.g., ["j", "Ctrl+n"]).
pub type CustomKeybindings = HashMap<String, Vec<String>>;

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Preferred TUI theme.
    #[serde(default)]
    pub theme: ThemeArg,

    /// Keybinding profile for the TUI.
    #[serde(default)]
    pub keybinding_profile: KeybindingProfile,

    /// Custom keybinding overrides.
    ///
    /// These bindings are merged with the selected profile's defaults.
    /// Custom bindings add to (not replace) the profile bindings.
    #[serde(default)]
    pub custom_keybindings: CustomKeybindings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeArg::Auto,
            keybinding_profile: KeybindingProfile::Universal,
            custom_keybindings: CustomKeybindings::new(),
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

    /// Check if custom keybindings are configured.
    #[must_use]
    pub fn has_custom_keybindings(&self) -> bool {
        !self.custom_keybindings.is_empty()
    }
}
