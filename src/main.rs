//! RustDupe - Smart Duplicate File Finder
//!
//! Entry point for the RustDupe CLI application.

use anyhow::Result;
use clap::Parser;
use directories::ProjectDirs;
use rustdupe::{
    cache::HashCache,
    cli::{Cli, Commands, LoadArgs, OutputFormat, ScanArgs, ScriptTypeArg, ThemeArg},
    config::Config,
    duplicates::{DuplicateFinder, FinderConfig},
    logging, output,
    scanner::WalkerConfig,
    session::{Session, SessionGroup, SessionSettings},
    signal,
    tui::keybindings::KeyBindings,
};
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;

fn main() -> Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Load configuration
    let mut config = Config::load();

    // Update config with CLI theme if provided (not the default Auto)
    if cli.theme != ThemeArg::Auto {
        config.theme = cli.theme;
        if let Err(e) = config.save() {
            log::warn!("Failed to save config: {}", e);
        }
    }

    // Use theme from CLI if provided, otherwise from config
    let theme = if cli.theme != ThemeArg::Auto {
        cli.theme
    } else {
        config.theme
    };

    // Determine if accessible mode should be enabled
    // CLI flag (--accessible) OR config setting OR NO_COLOR env var
    let accessible = cli.accessible || config.is_accessible();

    // Use keybinding profile from CLI if provided, otherwise from config
    // CLI flag (including env var) overrides config file
    let keybinding_profile = cli.keybinding_profile.unwrap_or(config.keybinding_profile);

    // Build KeyBindings with profile and custom overrides from config
    let keybindings = if config.has_custom_keybindings() {
        match KeyBindings::from_profile_with_custom(keybinding_profile, &config.custom_keybindings)
        {
            Ok(bindings) => bindings,
            Err(e) => {
                // Log warning but continue with profile defaults
                log::warn!(
                    "Invalid custom keybindings in config: {}. Using profile defaults.",
                    e
                );
                KeyBindings::from_profile(keybinding_profile)
            }
        }
    } else {
        KeyBindings::from_profile(keybinding_profile)
    };

    // Initialize logging based on verbosity flags (MUST be before any log calls)
    logging::init_logging(cli.verbose, cli.quiet);

    // Install signal handler for graceful shutdown (Ctrl+C)
    let shutdown_handler = signal::install_handler().map_err(|e| anyhow::anyhow!("{}", e))?;
    let shutdown_flag = shutdown_handler.get_flag();

    // Handle subcommands
    match cli.command {
        Commands::Scan(args) => handle_scan(
            *args,
            shutdown_flag,
            cli.quiet,
            theme,
            keybindings,
            accessible,
        ),
        Commands::Load(args) => handle_load(
            args,
            shutdown_flag,
            cli.quiet,
            theme,
            keybindings,
            accessible,
        ),
    }?;

    // Check if shutdown was requested and exit with appropriate code
    if shutdown_handler.is_shutdown_requested() {
        std::process::exit(signal::EXIT_CODE_INTERRUPTED);
    }

    Ok(())
}

fn handle_scan(
    args: ScanArgs,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    quiet: bool,
    theme: ThemeArg,
    keybindings: KeyBindings,
    accessible: bool,
) -> Result<()> {
    let (groups, summary, scan_paths, settings, reference_paths) = if let Some(ref session_path) =
        args.load_session
    {
        log::info!("Loading session from {:?}", session_path);
        let session = Session::load(session_path)?;
        let (groups, summary) = session.to_results();
        let reference_paths = groups
            .first()
            .map(|g| g.reference_paths.clone())
            .unwrap_or_default();
        (
            groups,
            summary,
            session.scan_paths,
            session.settings,
            reference_paths,
        )
    } else {
        let raw_path = args.path.as_ref().ok_or_else(|| {
            anyhow::anyhow!("A path is required for scanning unless --load-session is used")
        })?;

        // Canonicalize the scan path to ensure consistent path matching (especially on Windows)
        let path = raw_path.canonicalize()?;

        log::debug!("Scanning path: {:?}", path);
        log::debug!("Output format: {}", args.output);

        // Validate the path exists
        if !path.exists() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }

        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", path.display());
        }

        // Validate and canonicalize reference paths
        let mut reference_paths = Vec::new();
        for ref_path in args.reference_paths {
            if !ref_path.exists() {
                anyhow::bail!("Reference path does not exist: {}", ref_path.display());
            }
            if !ref_path.is_dir() {
                anyhow::bail!("Reference path is not a directory: {}", ref_path.display());
            }
            reference_paths.push(ref_path.canonicalize()?);
        }

        // Resolve cache path
        let cache_path = if let Some(path) = args.cache {
            path
        } else {
            let project_dirs = ProjectDirs::from("com", "rustdupe", "rustdupe")
                .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;
            let cache_dir = project_dirs.cache_dir();
            fs::create_dir_all(cache_dir)?;
            cache_dir.join("hashes.db")
        };

        // Initialize cache
        let hash_cache = if !args.no_cache {
            log::debug!("Using cache at: {:?}", cache_path);
            let cache = match HashCache::new(&cache_path) {
                Ok(cache) => Some(cache),
                Err(e) => {
                    log::warn!(
                        "Failed to open cache at {:?}: {}. Attempting recovery...",
                        cache_path,
                        e
                    );
                    if cache_path.exists() {
                        // Try to delete the corrupted cache and create a new one
                        if let Err(err) = fs::remove_file(&cache_path) {
                            log::error!(
                                "Failed to delete corrupted cache: {}. Caching disabled.",
                                err
                            );
                            None
                        } else {
                            match HashCache::new(&cache_path) {
                                Ok(cache) => {
                                    log::info!("Cache recovered successfully (reset to empty)");
                                    Some(cache)
                                }
                                Err(e2) => {
                                    log::error!(
                                        "Failed to recover cache: {}. Caching disabled.",
                                        e2
                                    );
                                    None
                                }
                            }
                        }
                    } else {
                        log::error!("Cache path does not exist but failed to initialize: {}. Caching disabled.", e);
                        None
                    }
                }
            };

            if let Some(ref cache) = cache {
                if args.clear_cache {
                    log::info!("Clearing cache...");
                    cache.clear()?;
                }
            }
            cache.map(Arc::new)
        } else {
            log::debug!("Caching is disabled");
            None
        };

        // Compile regex patterns
        let mut regex_include = Vec::new();
        for pattern in &args.regex_include {
            match regex::Regex::new(pattern) {
                Ok(re) => regex_include.push(re),
                Err(e) => anyhow::bail!("Invalid include regex '{}': {}", pattern, e),
            }
        }

        let mut regex_exclude = Vec::new();
        for pattern in &args.regex_exclude {
            match regex::Regex::new(pattern) {
                Ok(re) => regex_exclude.push(re),
                Err(e) => anyhow::bail!("Invalid exclude regex '{}': {}", pattern, e),
            }
        }

        // Configure the walker
        let walker_config = WalkerConfig::default()
            .with_follow_symlinks(args.follow_symlinks)
            .with_skip_hidden(args.skip_hidden)
            .with_min_size(args.min_size)
            .with_max_size(args.max_size)
            .with_newer_than(args.newer_than)
            .with_older_than(args.older_than)
            .with_patterns(args.ignore_patterns.clone())
            .with_regex_include(regex_include)
            .with_regex_exclude(regex_exclude)
            .with_file_categories(args.file_types.iter().map(|&t| t.into()).collect());

        // Configure progress reporting for non-TUI modes
        let progress = if args.output != OutputFormat::Tui {
            Some(Arc::new(rustdupe::progress::Progress::with_accessible(
                quiet, accessible,
            )))
        } else {
            None
        };

        // Configure the duplicate finder
        let mut finder_config = FinderConfig::default()
            .with_io_threads(args.io_threads)
            .with_paranoid(args.paranoid)
            .with_walker_config(walker_config)
            .with_shutdown_flag(shutdown_flag.clone())
            .with_reference_paths(reference_paths.clone());

        if let Some(cache) = hash_cache {
            finder_config = finder_config.with_cache(cache);
        }

        if let Some(ref p) = progress {
            finder_config = finder_config.with_progress_callback(
                p.clone() as Arc<dyn rustdupe::duplicates::ProgressCallback>
            );
        }

        let finder = DuplicateFinder::new(finder_config);

        log::info!("Starting scan of {}", path.display());

        match finder.find_duplicates(&path) {
            Ok((groups, summary)) => {
                let settings = SessionSettings {
                    follow_symlinks: args.follow_symlinks,
                    skip_hidden: args.skip_hidden,
                    min_size: args.min_size,
                    max_size: args.max_size,
                    newer_than: args.newer_than.map(chrono::DateTime::from),
                    older_than: args.older_than.map(chrono::DateTime::from),
                    ignore_patterns: args.ignore_patterns.clone(),
                    regex_include: args.regex_include.clone(),
                    regex_exclude: args.regex_exclude.clone(),
                    file_categories: args.file_types.iter().map(|&t| t.into()).collect(),
                    io_threads: args.io_threads,
                    paranoid: args.paranoid,
                };
                (
                    groups,
                    summary,
                    vec![path.clone()],
                    settings,
                    reference_paths,
                )
            }
            Err(e) => {
                if args.output == OutputFormat::Json {
                    let error_json = serde_json::json!({
                        "error": e.to_string(),
                        "interrupted": matches!(e, rustdupe::duplicates::FinderError::Interrupted)
                    });
                    eprintln!("{}", serde_json::to_string_pretty(&error_json)?);
                }
                anyhow::bail!("Scan failed: {}", e);
            }
        }
    };

    handle_results(ResultContext {
        groups,
        summary,
        output_format: args.output,
        output_file: args.output_file,
        script_type: args.script_type,
        save_session: args.save_session,
        scan_paths,
        settings,
        shutdown_flag,
        initial_session: None,
        reference_paths,
        dry_run: args.dry_run,
        theme,
        keybindings,
        accessible,
    })
}

fn handle_load(
    args: LoadArgs,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    _quiet: bool,
    theme: ThemeArg,
    keybindings: KeyBindings,
    accessible: bool,
) -> Result<()> {
    log::info!("Loading session from {:?}", args.path);
    let session = Session::load(&args.path)?;
    let (groups, summary) = session.to_results();
    let reference_paths = groups
        .first()
        .map(|g| g.reference_paths.clone())
        .unwrap_or_default();

    handle_results(ResultContext {
        groups,
        summary,
        output_format: args.output,
        output_file: args.output_file,
        script_type: args.script_type,
        save_session: None,
        scan_paths: session.scan_paths.clone(),
        settings: session.settings.clone(),
        shutdown_flag,
        initial_session: Some(session),
        reference_paths,
        dry_run: args.dry_run,
        theme,
        keybindings,
        accessible,
    })
}

struct ResultContext {
    groups: Vec<rustdupe::duplicates::DuplicateGroup>,
    summary: rustdupe::duplicates::ScanSummary,
    output_format: OutputFormat,
    output_file: Option<std::path::PathBuf>,
    script_type: Option<ScriptTypeArg>,
    save_session: Option<std::path::PathBuf>,
    scan_paths: Vec<std::path::PathBuf>,
    settings: SessionSettings,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    initial_session: Option<Session>,
    reference_paths: Vec<std::path::PathBuf>,
    dry_run: bool,
    theme: ThemeArg,
    keybindings: KeyBindings,
    accessible: bool,
}

fn handle_results(ctx: ResultContext) -> Result<()> {
    let ResultContext {
        groups,
        summary,
        output_format,
        output_file,
        script_type,
        save_session,
        scan_paths,
        settings,
        shutdown_flag,
        initial_session,
        reference_paths,
        dry_run,
        theme,
        keybindings,
        accessible,
    } = ctx;

    // 1. Save session if requested (non-TUI only)
    if output_format != OutputFormat::Tui {
        if let Some(ref path) = save_session {
            let session_groups = groups
                .iter()
                .enumerate()
                .map(|(id, g)| SessionGroup {
                    id,
                    hash: g.hash,
                    size: g.size,
                    files: g.files.clone(),
                    reference_paths: g.reference_paths.clone(),
                })
                .collect();
            let mut session = Session::new(scan_paths.clone(), settings.clone(), session_groups);
            if let Some(ref initial) = initial_session {
                session.user_selections = initial.user_selections.clone();
                session.group_index = initial.group_index;
                session.file_index = initial.file_index;
            }
            session.save(path)?;
            log::info!("Session saved to {:?}", path);
        }
    }

    // 2. Output results based on format
    match output_format {
        OutputFormat::Tui => {
            // Initialize TUI with results
            let mut app = rustdupe::tui::App::with_groups(groups)
                .with_reference_paths(reference_paths)
                .with_dry_run(dry_run)
                .with_theme(theme)
                .with_accessible(accessible);
            if let Some(session) = initial_session {
                app.apply_session(
                    session.user_selections,
                    session.group_index,
                    session.file_index,
                );
            }
            rustdupe::tui::run_tui_with_bindings(&mut app, Some(shutdown_flag), Some(keybindings))?;

            // Save session after TUI exit if requested
            if let Some(ref path) = save_session {
                let (group_index, file_index) = app.navigation_position();
                let session_groups = app
                    .groups()
                    .iter()
                    .enumerate()
                    .map(|(id, g)| SessionGroup {
                        id,
                        hash: g.hash,
                        size: g.size,
                        files: g.files.clone(),
                        reference_paths: g.reference_paths.clone(),
                    })
                    .collect();

                let mut session = Session::new(scan_paths, settings, session_groups);
                session.user_selections = app.selected_files_btree();
                session.group_index = group_index;
                session.file_index = file_index;
                session.save(path)?;
                log::info!("Session saved to {:?}", path);
            }
        }
        OutputFormat::Json => {
            let json_output = output::JsonOutput::new(&groups, &summary);
            if let Some(path) = output_file {
                let mut file = fs::File::create(&path)?;
                json_output.write_to(&mut file, true)?;
                file.flush()?;
                log::info!("JSON results saved to {:?}", path);
            } else {
                let mut stdout = io::stdout().lock();
                json_output.write_to(&mut stdout, true)?;
                stdout.flush()?;
            }
        }
        OutputFormat::Csv => {
            let csv_output = output::CsvOutput::new(&groups);
            if let Some(path) = output_file {
                let file = fs::File::create(&path)?;
                csv_output.write_to(file)?;
                log::info!("CSV results saved to {:?}", path);
            } else {
                let stdout = io::stdout().lock();
                csv_output.write_to(stdout)?;
            }
        }
        OutputFormat::Html => {
            let html_output = output::HtmlOutput::new(&groups, &summary);
            if let Some(path) = output_file {
                let mut file = fs::File::create(&path)?;
                html_output.write_to(&mut file)?;
                file.flush()?;
                log::info!("HTML report saved to {:?}", path);
            } else {
                let mut stdout = io::stdout().lock();
                html_output.write_to(&mut stdout)?;
                stdout.flush()?;
            }
        }
        OutputFormat::Session => {
            let session_groups = groups
                .iter()
                .enumerate()
                .map(|(id, g)| SessionGroup {
                    id,
                    hash: g.hash,
                    size: g.size,
                    files: g.files.clone(),
                    reference_paths: g.reference_paths.clone(),
                })
                .collect();

            let mut session = Session::new(scan_paths, settings, session_groups);
            if let Some(initial) = initial_session {
                session.user_selections = initial.user_selections;
                session.group_index = initial.group_index;
                session.file_index = initial.file_index;
            }
            let json = session.to_json()?;
            if let Some(path) = output_file {
                fs::write(&path, json)?;
                log::info!("Session saved to {:?}", path);
            } else {
                let mut stdout = io::stdout().lock();
                stdout.write_all(json.as_bytes())?;
                stdout.flush()?;
            }
        }
        OutputFormat::Script => {
            let script_type = match script_type {
                Some(ScriptTypeArg::Posix) => rustdupe::output::ScriptType::Posix,
                Some(ScriptTypeArg::Powershell) => rustdupe::output::ScriptType::PowerShell,
                None => rustdupe::output::ScriptType::detect(),
            };

            let mut script_output =
                rustdupe::output::ScriptOutput::new(&groups, &summary, script_type);

            // If we have an initial session with user selections, use them
            if let Some(ref session) = initial_session {
                script_output = script_output.with_user_selections(&session.user_selections);
            }

            if let Some(path) = output_file {
                let mut file = fs::File::create(&path)?;
                script_output.write_to(&mut file)?;
                file.flush()?;
                log::info!("Deletion script saved to {:?}", path);
            } else {
                let mut stdout = io::stdout().lock();
                script_output.write_to(&mut stdout)?;
                stdout.flush()?;
            }
        }
    }

    Ok(())
}
