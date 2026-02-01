//! Output formatters for duplicate scan results.
//!
//! This module provides different output formats for scan results:
//! - JSON for automation and scripting
//! - CSV for spreadsheet import
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::duplicates::{DuplicateFinder, DuplicateGroup, ScanSummary};
//! use rustdupe::output::json::JsonOutput;
//! use std::path::Path;
//!
//! let finder = DuplicateFinder::with_defaults();
//! let (groups, summary) = finder.find_duplicates(Path::new(".")).unwrap();
//!
//! // Output as JSON to stdout
//! let output = JsonOutput::new(&groups, &summary);
//! println!("{}", output.to_json_pretty().unwrap());
//! ```

pub mod csv;
pub mod html;
pub mod json;
pub mod script;

// Re-export main types
pub use csv::CsvOutput;
pub use html::HtmlOutput;
pub use json::JsonOutput;
pub use script::{ScriptOutput, ScriptType};
