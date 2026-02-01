//! RustDupe - Smart Duplicate File Finder
//!
//! Entry point for the RustDupe CLI application.

use anyhow::Result;
use clap::Parser;
use directories::ProjectDirs;
use rustdupe::{
    cache::HashCache,
    duplicates::{DuplicateFinder, FinderConfig},
    logging, output,
    scanner::WalkerConfig,
    session::{Session, SessionGroup, SessionSettings},
    signal,
};
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;

mod cli;

use cli::{Cli, Commands, LoadArgs, OutputFormat, ScanArgs};

fn main() -> Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Initialize logging based on verbosity flags (MUST be before any log calls)
    logging::init_logging(cli.verbose, cli.quiet);

    // Install signal handler for graceful shutdown (Ctrl+C)
    let shutdown_handler = signal::install_handler().map_err(|e| anyhow::anyhow!("{}", e))?;
    let shutdown_flag = shutdown_handler.get_flag();

    // Handle subcommands
    match cli.command {
        Commands::Scan(args) => handle_scan(args, shutdown_flag, cli.quiet),
        Commands::Load(args) => handle_load(args, shutdown_flag, cli.quiet),
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
) -> Result<()> {
    let (groups, summary, scan_paths, settings) = if let Some(ref session_path) = args.load_session
    {
        log::info!("Loading session from {:?}", session_path);
        let session = Session::load(session_path)?;
        let (groups, summary) = session.to_results();
        (groups, summary, session.scan_paths, session.settings)
    } else {
        let path = args.path.as_ref().ok_or_else(|| {
            anyhow::anyhow!("A path is required for scanning unless --load-session is used")
        })?;

        log::debug!("Scanning path: {:?}", path);
        log::debug!("Output format: {}", args.output);

        // Validate the path exists
        if !path.exists() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }

        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", path.display());
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
            let cache = HashCache::new(&cache_path)?;
            if args.clear_cache {
                log::info!("Clearing cache...");
                cache.clear()?;
            }
            Some(Arc::new(cache))
        } else {
            log::debug!("Caching is disabled");
            None
        };

        // Configure the walker
        let walker_config = WalkerConfig::default()
            .with_follow_symlinks(args.follow_symlinks)
            .with_skip_hidden(args.skip_hidden)
            .with_min_size(args.min_size)
            .with_max_size(args.max_size)
            .with_patterns(args.ignore_patterns.clone());

        // Configure progress reporting for non-TUI modes
        let progress = if args.output != OutputFormat::Tui {
            Some(Arc::new(rustdupe::progress::Progress::new(quiet)))
        } else {
            None
        };

        // Configure the duplicate finder
        let mut finder_config = FinderConfig::default()
            .with_io_threads(args.io_threads)
            .with_paranoid(args.paranoid)
            .with_walker_config(walker_config)
            .with_shutdown_flag(shutdown_flag.clone());

        if let Some(cache) = hash_cache {
            finder_config = finder_config.with_cache(cache);
        }

        if let Some(ref p) = progress {
            finder_config = finder_config.with_progress_callback(
                p.clone() as Arc<dyn rustdupe::duplicates::ProgressCallback>
            );
        }

        let finder = DuplicateFinder::new(finder_config);

        log::info!(
            "Starting scan of {}",
            if args.output == OutputFormat::Tui {
                path.canonicalize()?.display().to_string()
            } else {
                path.display().to_string()
            }
        );

        match finder.find_duplicates(path) {
            Ok((groups, summary)) => {
                let settings = SessionSettings {
                    follow_symlinks: args.follow_symlinks,
                    skip_hidden: args.skip_hidden,
                    min_size: args.min_size,
                    max_size: args.max_size,
                    ignore_patterns: args.ignore_patterns.clone(),
                    io_threads: args.io_threads,
                    paranoid: args.paranoid,
                };
                (groups, summary, vec![path.clone()], settings)
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
        save_session: args.save_session,
        scan_paths,
        settings,
        shutdown_flag,
        initial_session: None,
    })
}

fn handle_load(
    args: LoadArgs,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    _quiet: bool,
) -> Result<()> {
    log::info!("Loading session from {:?}", args.path);
    let session = Session::load(&args.path)?;
    let (groups, summary) = session.to_results();

    handle_results(ResultContext {
        groups,
        summary,
        output_format: args.output,
        save_session: None,
        scan_paths: session.scan_paths.clone(),
        settings: session.settings.clone(),
        shutdown_flag,
        initial_session: Some(session),
    })
}

struct ResultContext {
    groups: Vec<rustdupe::duplicates::DuplicateGroup>,
    summary: rustdupe::duplicates::ScanSummary,
    output_format: OutputFormat,
    save_session: Option<std::path::PathBuf>,
    scan_paths: Vec<std::path::PathBuf>,
    settings: SessionSettings,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    initial_session: Option<Session>,
}

fn handle_results(ctx: ResultContext) -> Result<()> {
    let ResultContext {
        groups,
        summary,
        output_format,
        save_session,
        scan_paths,
        settings,
        shutdown_flag,
        initial_session,
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
            let mut app = rustdupe::tui::App::with_groups(groups);
            if let Some(session) = initial_session {
                app.apply_session(
                    session.user_selections,
                    session.group_index,
                    session.file_index,
                );
            }
            rustdupe::tui::run_tui(&mut app, Some(shutdown_flag))?;

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
            let mut stdout = io::stdout().lock();
            json_output.write_to(&mut stdout, true)?;
            stdout.flush()?;
        }
        OutputFormat::Csv => {
            let csv_output = output::CsvOutput::new(&groups);
            let stdout = io::stdout().lock();
            csv_output.write_to(stdout)?;
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
                })
                .collect();

            let mut session = Session::new(scan_paths, settings, session_groups);
            if let Some(initial) = initial_session {
                session.user_selections = initial.user_selections;
                session.group_index = initial.group_index;
                session.file_index = initial.file_index;
            }
            let json = session.to_json()?;
            let mut stdout = io::stdout().lock();
            stdout.write_all(json.as_bytes())?;
            stdout.flush()?;
        }
    }

    Ok(())
}
