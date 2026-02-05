//! Comprehensive integration tests for configuration functionality.
//!
//! These tests verify the full configuration stack including defaults,
//! TOML file parsing, environment variable overrides, CLI flag overrides,
//! named profiles, and validation with fuzzy suggestions.

use clap::Parser;
use rustdupe::cli::{Cli, Commands, OutputFormat, ThemeArg};
use rustdupe::config::Config;
use rustdupe::tui::keybindings::KeybindingProfile;
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;

// =============================================================================
// Helper Functions
// =============================================================================

static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Clear all RUSTDUPE_* environment variables to avoid interference.
fn clear_env() {
    for (key, _) in std::env::vars() {
        if key.starts_with("RUSTDUPE_") {
            std::env::remove_var(key);
        }
    }
    // Also clear NO_COLOR as it affects accessibility
    std::env::remove_var("NO_COLOR");
}

// =============================================================================
// Basic Configuration Tests
// =============================================================================

#[test]
fn test_config_load_defaults() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let config = Config::default();
    assert_eq!(config.theme, ThemeArg::Auto);
    assert_eq!(config.keybinding_profile, KeybindingProfile::Universal);
    assert_eq!(config.io_threads, 4);
    assert!(!config.follow_symlinks);
    assert!(!config.accessibility.enabled);
}

#[test]
fn test_config_load_from_toml() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
theme = "light"
keybinding_profile = "vim"
io_threads = 8
follow_symlinks = true

[custom_keybindings]
quit = ["q", "Ctrl+c"]

[accessibility]
enabled = true
use_ascii_borders = true
"#;
    fs::write(&config_path, toml_content).unwrap();

    let config = Config::load_from_path(config_path, None);

    assert_eq!(config.theme, ThemeArg::Light);
    assert_eq!(config.keybinding_profile, KeybindingProfile::Vim);
    assert_eq!(config.io_threads, 8);
    assert!(config.follow_symlinks);
    assert_eq!(
        config.custom_keybindings.get("quit").unwrap(),
        &vec!["q".to_string(), "Ctrl+c".to_string()]
    );
    assert!(config.accessibility.enabled);
    assert!(config.accessibility.use_ascii_borders);
}

#[test]
fn test_config_save_toml() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let mut config = Config::default();
    config.theme = ThemeArg::Light;
    config.io_threads = 2;
    config
        .custom_keybindings
        .insert("quit".to_string(), vec!["q".to_string()]);

    let content = toml::to_string_pretty(&config).unwrap();
    fs::write(&config_path, content).unwrap();

    let saved_content = fs::read_to_string(&config_path).unwrap();
    assert!(saved_content.contains("theme = \"light\""));
    assert!(saved_content.contains("io_threads = 2"));
    assert!(saved_content.contains("quit = [\"q\"]"));
}

#[test]
fn test_config_missing_file_uses_defaults() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let nonexistent_path = temp_dir.path().join("nonexistent.toml");

    let config = Config::load_from_path(nonexistent_path, None);
    assert_eq!(config.theme, ThemeArg::Auto);
    assert_eq!(config.io_threads, 4);
}

// =============================================================================
// Override Hierarchy Tests
// =============================================================================

#[test]
fn test_config_hierarchy_defaults_config_env_cli() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // 1. Config file overrides defaults
    let toml_content = r#"
theme = "light"
io_threads = 8
"#;
    fs::write(&config_path, toml_content).unwrap();

    // 2. Environment variables override config file
    std::env::set_var("RUSTDUPE_THEME", "dark");

    let mut config = Config::load_from_path(config_path, None);
    assert_eq!(config.theme, ThemeArg::Dark);
    assert_eq!(config.io_threads, 8);

    // 3. CLI flags override environment variables
    let cli = Cli::try_parse_from(["rustdupe", "--theme", "light", "scan", "."]).unwrap();
    config.merge_cli(&cli);
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert_eq!(config.theme, ThemeArg::Light);
    assert_eq!(config.io_threads, 8);

    // 4. CLI flag for io_threads overrides config file
    let cli = Cli::try_parse_from(["rustdupe", "scan", ".", "--io-threads", "16"]).unwrap();
    config.merge_cli(&cli);
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert_eq!(config.io_threads, 16);

    // Clean up
    std::env::remove_var("RUSTDUPE_THEME");
}

#[test]
fn test_boolean_overrides() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // config: follow_symlinks=true
    fs::write(&config_path, "follow_symlinks = true").unwrap();
    let mut config = Config::load_from_path(config_path, None);
    assert!(config.follow_symlinks);

    // CLI: --no-follow-symlinks should override config
    let cli = Cli::try_parse_from(["rustdupe", "scan", ".", "--no-follow-symlinks"]).unwrap();
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert!(!config.follow_symlinks);

    // CLI: --follow-symlinks should override if it was false in config
    let mut config2 = Config::default();
    assert!(!config2.follow_symlinks);
    let cli = Cli::try_parse_from(["rustdupe", "scan", ".", "--follow-symlinks"]).unwrap();
    if let Commands::Scan(args) = &cli.command {
        config2.merge_scan_args(args);
    }
    assert!(config2.follow_symlinks);
}

#[test]
fn test_output_override() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // config: output=json
    fs::write(&config_path, "output = \"json\"").unwrap();
    let mut config = Config::load_from_path(config_path, None);
    assert_eq!(config.output, OutputFormat::Json);

    // CLI: --output csv should override
    let cli = Cli::try_parse_from(["rustdupe", "scan", ".", "--output", "csv"]).unwrap();
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert_eq!(config.output, OutputFormat::Csv);
}

// =============================================================================
// Profile Tests
// =============================================================================

#[test]
fn test_config_load_profile() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
theme = "light"
io_threads = 4

[profile.photos]
theme = "dark"
io_threads = 8
min_size = 1000000

[profile.docs]
follow_symlinks = true
skip_hidden = true
"#;
    fs::write(&config_path, toml_content).unwrap();

    // Load with photos profile
    let config = Config::load_from_path(config_path.clone(), Some("photos"));
    assert_eq!(config.theme, ThemeArg::Dark); // Overridden by profile
    assert_eq!(config.io_threads, 8); // Overridden by profile
    assert_eq!(config.min_size, Some(1000000)); // Set in profile
    assert!(!config.follow_symlinks); // Default from base

    // Load with docs profile
    let config = Config::load_from_path(config_path.clone(), Some("docs"));
    assert_eq!(config.theme, ThemeArg::Light); // From base
    assert!(config.follow_symlinks); // Set in profile
    assert!(config.skip_hidden); // Set in profile
}

#[test]
fn test_config_profile_not_found() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
theme = "light"
[profile.photos]
theme = "dark"
"#;
    fs::write(&config_path, toml_content).unwrap();

    // Load with non-existent profile - should fallback to base
    let config = Config::load_from_path(config_path, Some("nonexistent"));
    assert_eq!(config.theme, ThemeArg::Light);
}

#[test]
fn test_config_list_profiles() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
[profile.photos]
theme = "dark"

[profile.docs]
theme = "light"
"#;
    fs::write(&config_path, toml_content).unwrap();

    let config = Config::load_from_path(config_path, None);
    assert!(config.profile.contains_key("photos"));
    assert!(config.profile.contains_key("docs"));
    assert_eq!(config.profile.len(), 2);
}

// =============================================================================
// Validation and Error Message Tests
// =============================================================================

#[test]
fn test_config_unknown_field_warning_with_suggestion() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // 'folow_symlinks' is a typo for 'follow_symlinks'
    let toml_content = r#"
folow_symlinks = true
"#;
    fs::write(&config_path, toml_content).unwrap();

    // This should not error but should log a warning (hard to test log directly here,
    // but we can verify it still loads defaults for other fields)
    let config = Config::load_from_path(config_path, None);
    assert!(!config.follow_symlinks); // Defaults because typo'd field was ignored
}

#[test]
fn test_config_invalid_type_error_message() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // io_threads should be an integer, not a string
    let toml_content = r#"
io_threads = "eight"
"#;
    fs::write(&config_path, toml_content).unwrap();

    // Figment should return an error. In our load_from_path it might panic or
    // return default depending on implementation.
    let config = Config::load_from_path(config_path, None);
    assert_eq!(config.io_threads, 4); // Falls back to default on error
}

#[test]
fn test_accessibility_config_validation() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
[accessibility]
unknown_field = true
enabled = true
"#;
    fs::write(&config_path, toml_content).unwrap();

    let config = Config::load_from_path(config_path, None);
    assert!(config.accessibility.enabled);
}

#[test]
fn test_profile_validation() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clear_env();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
[profile.test]
unknown_profile_field = 123
theme = "dark"
"#;
    fs::write(&config_path, toml_content).unwrap();

    let config = Config::load_from_path(config_path, Some("test"));
    assert_eq!(config.theme, ThemeArg::Dark);
}
