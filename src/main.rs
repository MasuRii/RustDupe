//! RustDupe - Smart Duplicate File Finder
//!
//! Entry point for the RustDupe CLI application.

use clap::Parser;
use rustdupe::{
    cli::Cli,
    error::{ExitCode, StructuredError},
};

fn main() {
    // Parse command-line arguments
    let cli = Cli::parse();
    let json_errors = cli.json_errors;

    // Run the application logic
    match rustdupe::run_app(cli) {
        Ok(code) => std::process::exit(code.as_i32()),
        Err(err) => {
            // Determine appropriate exit code for errors
            let exit_code = if err
                .downcast_ref::<rustdupe::duplicates::FinderError>()
                .is_some_and(|e| matches!(e, rustdupe::duplicates::FinderError::Interrupted))
            {
                ExitCode::Interrupted
            } else {
                ExitCode::GeneralError
            };

            // Report the error
            if json_errors {
                let structured = StructuredError::new(&err, exit_code);
                if let Ok(json) = serde_json::to_string_pretty(&structured) {
                    eprintln!("{}", json);
                } else {
                    eprintln!("[{}] Error: {}", exit_code.code_prefix(), err);
                }
            } else {
                eprintln!("[{}] Error: {}", exit_code.code_prefix(), err);
            }

            std::process::exit(exit_code.as_i32());
        }
    }
}
