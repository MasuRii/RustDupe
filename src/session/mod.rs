//! Session module for persisting scan results and user selections.
//!
//! This module provides functionality to save and load scan sessions,
//! allowing users to resume their work across different runs.

pub mod data;
pub mod io;

pub use data::Session;
