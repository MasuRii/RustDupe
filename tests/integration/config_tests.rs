use figment::providers::Serialized;
use rustdupe::cli::ThemeArg;
use rustdupe::config::Config;
use rustdupe::tui::keybindings::KeybindingProfile;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_config_load_defaults() {
    // Use figment directly without Env to avoid interference from other tests
    let figment = figment::Figment::from(Serialized::defaults(Config::default()));
    let config: Config = figment.extract().unwrap();
    assert_eq!(config.theme, ThemeArg::Auto);
    assert_eq!(config.keybinding_profile, KeybindingProfile::Universal);
    assert_eq!(config.io_threads, 4);
}

#[test]
fn test_config_load_from_env() {
    std::env::set_var("RUSTDUPE_THEME", "dark");
    std::env::set_var("RUSTDUPE_IO_THREADS", "16");
    // Use double underscore for nesting
    std::env::set_var("RUSTDUPE_ACCESSIBILITY__ENABLED", "true");

    // Use figment directly to test loading from environment
    use figment::{providers::Env, Figment};
    let figment = Figment::from(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("RUSTDUPE_").split("__"));

    let config: Config = figment.extract().unwrap();

    assert_eq!(config.theme, ThemeArg::Dark);
    assert_eq!(config.io_threads, 16);
    assert!(config.accessibility.enabled);

    // Clean up
    std::env::remove_var("RUSTDUPE_THEME");
    std::env::remove_var("RUSTDUPE_IO_THREADS");
    std::env::remove_var("RUSTDUPE_ACCESSIBILITY__ENABLED");
}

#[test]
fn test_config_load_from_toml() {
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
use_ascii_borders = false
"#;
    fs::write(&config_path, toml_content).unwrap();

    // Use figment directly to test loading from this specific file
    use figment::{
        providers::{Format, Toml},
        Figment,
    };
    let figment =
        Figment::from(Serialized::defaults(Config::default())).merge(Toml::file(&config_path));

    let config: Config = figment.extract().unwrap();

    assert_eq!(config.theme, ThemeArg::Light);
    assert_eq!(config.keybinding_profile, KeybindingProfile::Vim);
    assert_eq!(config.io_threads, 8);
    assert!(config.follow_symlinks);
    assert_eq!(
        config.custom_keybindings.get("quit").unwrap(),
        &vec!["q".to_string(), "Ctrl+c".to_string()]
    );
    assert!(config.accessibility.enabled);
    assert!(!config.accessibility.use_ascii_borders);
}

#[test]
fn test_config_save_toml() {
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
fn test_config_invalid_toml_fallback() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = "invalid = toml";
    fs::write(&config_path, toml_content).unwrap();

    // Figment should return error on invalid TOML format
    use figment::{
        providers::{Format, Toml},
        Figment,
    };
    let figment =
        Figment::from(Serialized::defaults(Config::default())).merge(Toml::file(&config_path));

    let result: Result<Config, _> = figment.extract();
    assert!(result.is_err());
}
