//! Application configuration management.
//!
//! This module handles loading and saving application-wide configuration
//! settings using a layered approach (defaults, config file, environment variables).
//!
//! # Configuration Hierarchy
//!
//! 1. CLI arguments (highest priority, handled in main.rs)
//! 2. Environment variables (`RUSTDUPE_*`)
//! 3. Configuration file (`config.toml`)
//! 4. Default values (lowest priority)
//!
//! # Custom Keybindings
//!
//! Custom keybindings can be defined in the config file using the `custom_keybindings`
//! section. Each entry maps an action name to a list of key specifications:
//!
//! ```toml
//! [custom_keybindings]
//! navigate_down = ["n", "Ctrl+n"]
//! quit = ["x", "Ctrl+q"]
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
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::cli::{FileType, OutputFormat, ThemeArg};
use crate::tui::keybindings::KeybindingProfile;

/// Type alias for custom keybinding overrides.
///
/// Maps action names (e.g., "navigate_down") to lists of key specifications
/// (e.g., ["j", "Ctrl+n"]).
pub type CustomKeybindings = HashMap<String, Vec<String>>;

/// Accessibility settings for screen reader compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    /// Enable accessible mode (overridden by --accessible CLI flag).
    #[serde(default)]
    pub enabled: bool,

    /// Use simple ASCII borders instead of Unicode box-drawing characters.
    #[serde(default = "default_true")]
    pub use_ascii_borders: bool,

    /// Disable animations and spinners for screen reader compatibility.
    #[serde(default = "default_true")]
    pub disable_animations: bool,

    /// Use simplified progress output without cursor movement.
    #[serde(default = "default_true")]
    pub simplified_progress: bool,

    /// Reduce screen refresh rate for better screen reader performance.
    #[serde(default = "default_true")]
    pub reduce_refresh_rate: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            use_ascii_borders: true,
            disable_animations: true,
            simplified_progress: true,
            reduce_refresh_rate: true,
        }
    }
}

impl AccessibilityConfig {
    /// Check if accessible mode is active.
    ///
    /// Also returns true if NO_COLOR environment variable is set,
    /// as this often indicates a screen reader environment.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.enabled || std::env::var("NO_COLOR").is_ok()
    }

    /// Create an accessibility config with accessible mode enabled.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // TUI & Appearance
    /// Preferred TUI theme.
    #[serde(default)]
    pub theme: ThemeArg,

    /// Keybinding profile for the TUI.
    #[serde(default)]
    pub keybinding_profile: KeybindingProfile,

    /// Custom keybinding overrides.
    #[serde(default)]
    pub custom_keybindings: CustomKeybindings,

    /// Accessibility settings.
    #[serde(default)]
    pub accessibility: AccessibilityConfig,

    // Scanning Defaults
    /// Follow symbolic links during scan.
    #[serde(default)]
    pub follow_symlinks: bool,

    /// Skip hidden files and directories.
    #[serde(default)]
    pub skip_hidden: bool,

    /// Minimum file size to consider.
    #[serde(default)]
    pub min_size: Option<u64>,

    /// Maximum file size to consider.
    #[serde(default)]
    pub max_size: Option<u64>,

    /// Only include files modified after this date.
    #[serde(default)]
    pub newer_than: Option<chrono::DateTime<chrono::Utc>>,

    /// Only include files modified before this date.
    #[serde(default)]
    pub older_than: Option<chrono::DateTime<chrono::Utc>>,

    /// Number of I/O threads for hashing.
    #[serde(default = "default_io_threads")]
    pub io_threads: usize,

    /// Enable paranoid mode (byte-by-byte verification).
    #[serde(default)]
    pub paranoid: bool,

    // Filtering Defaults
    /// Glob patterns to ignore.
    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    /// Regex patterns to include.
    #[serde(default)]
    pub regex_include: Vec<String>,

    /// Regex patterns to exclude.
    #[serde(default)]
    pub regex_exclude: Vec<String>,

    /// Filter by file type categories.
    #[serde(default)]
    pub file_types: Vec<FileType>,

    // Cache Defaults
    /// Disable hash caching.
    #[serde(default)]
    pub no_cache: bool,

    /// Path to the hash cache database.
    #[serde(default)]
    pub cache: Option<PathBuf>,

    // Safety & Deletion Defaults
    /// Use permanent deletion instead of moving to trash.
    #[serde(default)]
    pub permanent: bool,

    /// Do not perform any deletions (read-only mode).
    #[serde(default)]
    pub dry_run: bool,

    // Output Defaults
    /// Default output format.
    #[serde(default)]
    pub output: OutputFormat,
}

fn default_io_threads() -> usize {
    4
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeArg::Auto,
            keybinding_profile: KeybindingProfile::Universal,
            custom_keybindings: CustomKeybindings::new(),
            accessibility: AccessibilityConfig::default(),
            follow_symlinks: false,
            skip_hidden: false,
            min_size: None,
            max_size: None,
            newer_than: None,
            older_than: None,
            io_threads: 4,
            paranoid: false,
            ignore_patterns: Vec::new(),
            regex_include: Vec::new(),
            regex_exclude: Vec::new(),
            file_types: Vec::new(),
            no_cache: false,
            cache: None,
            permanent: false,
            dry_run: false,
            output: OutputFormat::Tui,
        }
    }
}

impl Config {
    /// Load the configuration using figment for layered support.
    ///
    /// Layers (from highest to lowest priority):
    /// 1. Environment variables prefixed with `RUSTDUPE_`
    /// 2. Configuration file (`config.toml`)
    /// 3. Defaults
    pub fn load() -> Self {
        Self::load_from_path(Self::config_path().unwrap_or_default())
    }

    /// Load configuration from a specific path.
    pub fn load_from_path(path: PathBuf) -> Self {
        let figment = Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file(&path))
            .merge(Env::prefixed("RUSTDUPE_").split("__"));

        match figment.extract::<Self>() {
            Ok(config) => config,
            Err(e) => {
                // If there's an error, log it and return defaults.
                eprintln!(
                    "Warning: Failed to load configuration: {}. Using defaults.",
                    e
                );
                Self::default()
            }
        }
    }

    /// Save the configuration to the default platform-specific path (TOML format).
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default platform-specific configuration path (config.toml).
    pub fn config_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "rustdupe", "rustdupe")
            .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;
        Ok(project_dirs.config_dir().join("config.toml"))
    }

    /// Check if custom keybindings are configured.
    #[must_use]
    pub fn has_custom_keybindings(&self) -> bool {
        !self.custom_keybindings.is_empty()
    }

    /// Check if accessible mode is active.
    #[must_use]
    pub fn is_accessible(&self) -> bool {
        self.accessibility.is_active()
    }

    /// Enable accessible mode.
    pub fn enable_accessibility(&mut self) {
        self.accessibility.enabled = true;
    }

    /// Merge global CLI arguments into the configuration.
    pub fn merge_cli(&mut self, cli: &crate::cli::Cli) {
        if let Some(theme) = cli.theme {
            self.theme = theme;
        }
        if let Some(profile) = cli.keybinding_profile {
            self.keybinding_profile = profile;
        }
        if cli.accessible {
            self.accessibility.enabled = true;
        }
        if cli.no_accessible {
            self.accessibility.enabled = false;
        }
        if cli.no_color {
            std::env::set_var("NO_COLOR", "1");
        }
        if cli.color {
            std::env::remove_var("NO_COLOR");
        }
    }

    /// Merge scan arguments into the configuration.
    pub fn merge_scan_args(&mut self, args: &crate::cli::ScanArgs) {
        if args.follow_symlinks {
            self.follow_symlinks = true;
        }
        if args.no_follow_symlinks {
            self.follow_symlinks = false;
        }
        if args.skip_hidden {
            self.skip_hidden = true;
        }
        if args.no_skip_hidden {
            self.skip_hidden = false;
        }
        if let Some(min) = args.min_size {
            self.min_size = Some(min);
        }
        if let Some(max) = args.max_size {
            self.max_size = Some(max);
        }
        if let Some(newer) = args.newer_than {
            self.newer_than = Some(chrono::DateTime::from(newer));
        }
        if let Some(older) = args.older_than {
            self.older_than = Some(chrono::DateTime::from(older));
        }
        if let Some(threads) = args.io_threads {
            self.io_threads = threads;
        }
        if args.paranoid {
            self.paranoid = true;
        }
        if args.no_paranoid {
            self.paranoid = false;
        }
        if !args.ignore_patterns.is_empty() {
            self.ignore_patterns = args.ignore_patterns.clone();
        }
        if !args.regex_include.is_empty() {
            self.regex_include = args.regex_include.clone();
        }
        if !args.regex_exclude.is_empty() {
            self.regex_exclude = args.regex_exclude.clone();
        }
        if !args.file_types.is_empty() {
            self.file_types = args.file_types.clone();
        }
        if args.no_cache {
            self.no_cache = true;
        }
        if args.enable_cache {
            self.no_cache = false;
        }
        if let Some(cache) = &args.cache {
            self.cache = Some(cache.clone());
        }
        if args.permanent {
            self.permanent = true;
        }
        if args.no_permanent {
            self.permanent = false;
        }
        if args.dry_run {
            self.dry_run = true;
        }
        if args.no_dry_run {
            self.dry_run = false;
        }
        if let Some(output) = args.output {
            self.output = output;
        }
    }

    /// Merge load arguments into the configuration.
    pub fn merge_load_args(&mut self, args: &crate::cli::LoadArgs) {
        if args.dry_run {
            self.dry_run = true;
        }
        if args.no_dry_run {
            self.dry_run = false;
        }
        if let Some(output) = args.output {
            self.output = output;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.theme, ThemeArg::Auto);
        assert_eq!(config.io_threads, 4);
        assert!(!config.follow_symlinks);
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path().unwrap();
        assert!(path.to_string_lossy().contains("rustdupe"));
        assert!(path.ends_with("config.toml"));
    }
}
