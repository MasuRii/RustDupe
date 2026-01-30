//! Duplicate detection module.
//!
//! This module provides functionality for:
//! - Size-based file grouping (Phase 1)
//! - Prehash comparison (Phase 2)
//! - Full hash comparison (Phase 3)
//! - Duplicate group management

pub mod finder;
pub mod groups;
