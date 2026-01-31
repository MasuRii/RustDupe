//! RustDupe - Smart Duplicate File Finder
//!
//! Entry point for the RustDupe CLI application.

use anyhow::Result;
use clap::Parser;
use rustdupe::{
    duplicates::{DuplicateFinder, FinderConfig},
    logging, output,
    scanner::WalkerConfig,
    signal,
};
use std::io::{self, Write};
use std::sync::Arc;

mod cli;

use cli::{Cli, Commands, OutputFormat};

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
        Commands::Scan(args) => {
            log::debug!("Scanning path: {:?}", args.path);
            log::debug!("Output format: {}", args.output);

            // Validate the path exists
            if !args.path.exists() {
                anyhow::bail!("Path does not exist: {}", args.path.display());
            }

            if !args.path.is_dir() {
                anyhow::bail!("Path is not a directory: {}", args.path.display());
            }

            // Log configuration
            if let Some(min) = args.min_size {
                log::debug!("Minimum file size: {} bytes", min);
            }
            if let Some(max) = args.max_size {
                log::debug!("Maximum file size: {} bytes", max);
            }
            if !args.ignore_patterns.is_empty() {
                log::debug!("Ignore patterns: {:?}", args.ignore_patterns);
            }

            // Configure the walker
            let walker_config = WalkerConfig::default()
                .with_follow_symlinks(args.follow_symlinks)
                .with_skip_hidden(args.skip_hidden)
                .with_min_size(args.min_size)
                .with_max_size(args.max_size)
                .with_patterns(args.ignore_patterns.clone());

            // Configure progress reporting for non-TUI modes
            let progress = if args.output != OutputFormat::Tui {
                Some(Arc::new(rustdupe::progress::Progress::new(cli.quiet)))
            } else {
                None
            };

            // Configure the duplicate finder
            let mut finder_config = FinderConfig::default()
                .with_io_threads(args.io_threads)
                .with_paranoid(args.paranoid)
                .with_walker_config(walker_config)
                .with_shutdown_flag(shutdown_flag.clone());

            if let Some(ref p) = progress {
                finder_config = finder_config.with_progress_callback(
                    p.clone() as Arc<dyn rustdupe::duplicates::ProgressCallback>
                );
            }

            let finder = DuplicateFinder::new(finder_config);

            // Run the scan based on output format
            match args.output {
                OutputFormat::Tui => {
                    log::info!(
                        "Starting TUI scan of {}",
                        args.path.canonicalize()?.display()
                    );

                    // In TUI mode, we run the scan first, then launch the interactive UI
                    // TODO: Move scan inside run_tui for live progress updates
                    match finder.find_duplicates(&args.path) {
                        Ok((groups, summary)) => {
                            log::info!(
                                "Scan complete: {} groups, {} reclaimable",
                                summary.duplicate_groups,
                                summary.reclaimable_display()
                            );

                            // Initialize TUI with results
                            let app = rustdupe::tui::App::with_groups(groups);
                            rustdupe::tui::run_tui(app, Some(shutdown_flag.clone()))?;
                        }
                        Err(e) => {
                            anyhow::bail!("Scan failed: {}", e);
                        }
                    }
                }
                OutputFormat::Json => {
                    log::info!("Starting JSON scan of {}", args.path.display());
                    match finder.find_duplicates(&args.path) {
                        Ok((groups, summary)) => {
                            let json_output = output::JsonOutput::new(&groups, &summary);
                            let mut stdout = io::stdout().lock();
                            json_output.write_to(&mut stdout, true)?;
                            stdout.flush()?;
                        }
                        Err(e) => {
                            // Output error as JSON to stderr, but also return error
                            let error_json = serde_json::json!({
                                "error": e.to_string(),
                                "interrupted": matches!(e, rustdupe::duplicates::FinderError::Interrupted)
                            });
                            eprintln!("{}", serde_json::to_string_pretty(&error_json)?);
                            anyhow::bail!("Scan failed: {}", e);
                        }
                    }
                }
                OutputFormat::Csv => {
                    log::info!("Starting CSV scan of {}", args.path.display());
                    match finder.find_duplicates(&args.path) {
                        Ok((groups, _summary)) => {
                            let csv_output = output::CsvOutput::new(&groups);
                            let stdout = io::stdout().lock();
                            csv_output.write_to(stdout)?;
                        }
                        Err(e) => {
                            anyhow::bail!("Scan failed: {}", e);
                        }
                    }
                }
            }

            // Check if shutdown was requested and exit with appropriate code
            if shutdown_handler.is_shutdown_requested() {
                std::process::exit(signal::EXIT_CODE_INTERRUPTED);
            }

            Ok(())
        }
    }
}
