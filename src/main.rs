//! RustDupe - Smart Duplicate File Finder
//!
//! Entry point for the RustDupe CLI application.

use anyhow::Result;
use clap::Parser;
use rustdupe::logging;

mod cli;

use cli::{Cli, Commands, OutputFormat};

fn main() -> Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Initialize logging based on verbosity flags (MUST be before any log calls)
    logging::init_logging(cli.verbose, cli.quiet);

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

            // TODO: Implement actual scanning logic (Task 3.2.1+)
            match args.output {
                OutputFormat::Tui => {
                    log::info!(
                        "Starting TUI scan of {}",
                        args.path.canonicalize()?.display()
                    );
                    // TODO: Launch TUI (Task 3.4.x)
                    println!(
                        "TUI mode not yet implemented. Scanning: {}",
                        args.path.display()
                    );
                }
                OutputFormat::Json => {
                    log::info!("Starting JSON scan of {}", args.path.display());
                    // TODO: Implement JSON output (Task 3.6.2)
                    println!(
                        "{{\"status\": \"not_implemented\", \"path\": \"{}\"}}",
                        args.path.display()
                    );
                }
                OutputFormat::Csv => {
                    log::info!("Starting CSV scan of {}", args.path.display());
                    // TODO: Implement CSV output (Task 3.6.3)
                    println!("status,path");
                    println!("not_implemented,{}", args.path.display());
                }
            }

            Ok(())
        }
    }
}
