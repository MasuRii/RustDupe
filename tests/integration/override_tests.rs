use clap::Parser;
use rustdupe::cli::{Cli, Commands, OutputFormat, ThemeArg};
use rustdupe::config::Config;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_hierarchy_defaults_config_env_cli() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // 1. Defaults: theme=auto, io_threads=4, follow_symlinks=false
    let mut config = Config::default();
    assert_eq!(config.theme, ThemeArg::Auto);
    assert_eq!(config.io_threads, 4);
    assert!(!config.follow_symlinks);

    // 2. Config file overrides defaults: theme=light, io_threads=8
    let toml_content = r#"
theme = "light"
io_threads = 8
"#;
    fs::write(&config_path, toml_content).unwrap();

    config = Config::load_from_path(config_path.clone(), None);
    assert_eq!(config.theme, ThemeArg::Light);
    assert_eq!(config.io_threads, 8);
    assert!(!config.follow_symlinks); // still false

    // 3. Environment variables override config file: theme=dark
    std::env::set_var("RUSTDUPE_THEME", "dark");
    config = Config::load_from_path(config_path.clone(), None);
    assert_eq!(config.theme, ThemeArg::Dark);
    assert_eq!(config.io_threads, 8); // still from config file

    // 4. CLI flags override environment variables: theme=light (via CLI)
    // We simulate CLI parsing and merging
    let cli = Cli::try_parse_from(["rustdupe", "--theme", "light", "scan", "."]).unwrap();
    config.merge_cli(&cli);
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert_eq!(config.theme, ThemeArg::Light);
    assert_eq!(config.io_threads, 8);

    // 5. CLI flag for io_threads overrides config file
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
    // defaults: follow_symlinks=false
    let mut config = Config::default();
    assert!(!config.follow_symlinks);

    // config: follow_symlinks=true
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "follow_symlinks = true").unwrap();
    config = Config::load_from_path(config_path, None);
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
    let mut config = Config::default();
    assert_eq!(config.output, OutputFormat::Tui);

    // config: output=json
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "output = \"json\"").unwrap();
    config = Config::load_from_path(config_path, None);
    assert_eq!(config.output, OutputFormat::Json);

    // CLI: no output specified, should stay json
    let cli = Cli::try_parse_from(["rustdupe", "scan", "."]).unwrap();
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert_eq!(config.output, OutputFormat::Json);

    // CLI: --output csv should override
    let cli = Cli::try_parse_from(["rustdupe", "scan", ".", "--output", "csv"]).unwrap();
    if let Commands::Scan(args) = &cli.command {
        config.merge_scan_args(args);
    }
    assert_eq!(config.output, OutputFormat::Csv);
}
