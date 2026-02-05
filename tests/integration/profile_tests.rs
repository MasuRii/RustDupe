use rustdupe::cli::ThemeArg;
use rustdupe::config::Config;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_config_load_profile() {
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
    assert_eq!(config.min_size, None); // Not set anywhere
}

#[test]
fn test_config_profile_not_found() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
theme = "light"
[profile.photos]
theme = "dark"
"#;
    fs::write(&config_path, toml_content).unwrap();

    // Load with non-existent profile - should fallback to base and warn
    let config = Config::load_from_path(config_path, Some("nonexistent"));
    assert_eq!(config.theme, ThemeArg::Light);
}

#[test]
fn test_config_list_profiles() {
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
