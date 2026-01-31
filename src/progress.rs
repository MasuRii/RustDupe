//! Progress reporting utilities using indicatif.
//!
//! This module provides the [`Progress`] struct which implements [`ProgressCallback`]
//! to display visual progress bars in the terminal for non-TUI output modes.

use std::sync::Mutex;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::duplicates::ProgressCallback;

/// Progress reporter using indicatif.
///
/// Manages multiple progress bars for different phases of the duplicate
/// detection pipeline.
pub struct Progress {
    multi: MultiProgress,
    walking: Mutex<Option<ProgressBar>>,
    prehash: Mutex<Option<ProgressBar>>,
    fullhash: Mutex<Option<ProgressBar>>,
    quiet: bool,
}

impl Progress {
    /// Create a new progress reporter.
    ///
    /// # Arguments
    ///
    /// * `quiet` - If true, no progress bars will be displayed.
    #[must_use]
    pub fn new(quiet: bool) -> Self {
        Self {
            multi: MultiProgress::new(),
            walking: Mutex::new(None),
            prehash: Mutex::new(None),
            fullhash: Mutex::new(None),
            quiet,
        }
    }

    /// Create a style for the walking phase (spinner).
    fn walking_style() -> ProgressStyle {
        ProgressStyle::with_template("{spinner:.green} {msg} [{elapsed_precise}] {pos} files")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
    }

    /// Create a style for the prehash phase (progress bar).
    fn prehash_style() -> ProgressStyle {
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg} (ETA: {eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("█>-")
    }

    /// Create a style for the fullhash phase (progress bar with throughput).
    fn fullhash_style() -> ProgressStyle {
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{bar:40.green/blue}] {pos}/{len} ({percent}%) {msg} {per_sec} (ETA: {eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("█>-")
    }
}

impl ProgressCallback for Progress {
    fn on_phase_start(&self, phase: &str, total: usize) {
        if self.quiet {
            return;
        }

        match phase {
            "walking" => {
                let pb = self.multi.add(ProgressBar::new_spinner());
                pb.set_style(Self::walking_style());
                pb.set_message("Walking directory");
                pb.enable_steady_tick(Duration::from_millis(100));
                let mut walking = self.walking.lock().unwrap();
                *walking = Some(pb);
            }
            "prehash" => {
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(Self::prehash_style());
                pb.set_message("Prehashing");
                let mut prehash = self.prehash.lock().unwrap();
                *prehash = Some(pb);
            }
            "fullhash" => {
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(Self::fullhash_style());
                pb.set_message("Full hashing");
                let mut fullhash = self.fullhash.lock().unwrap();
                *fullhash = Some(pb);
            }
            _ => {
                // For any other phase, use a default bar
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(Self::prehash_style());
                pb.set_message(phase.to_string());
                // We don't store these for now, or we could have a map
            }
        }
    }

    fn on_progress(&self, current: usize, path: &str) {
        if self.quiet {
            return;
        }

        // Update the active progress bar
        // We check which one is active. Usually only one is active at a time
        // in the current DuplicateFinder implementation.

        if let Some(ref pb) = *self.fullhash.lock().unwrap() {
            pb.set_position(current as u64);
            pb.set_message(truncate_path(path, 30));
        } else if let Some(ref pb) = *self.prehash.lock().unwrap() {
            pb.set_position(current as u64);
            pb.set_message(truncate_path(path, 30));
        } else if let Some(ref pb) = *self.walking.lock().unwrap() {
            pb.set_position(current as u64);
            pb.set_message(truncate_path(path, 30));
        }
    }

    fn on_item_completed(&self, _bytes: u64) {
        // We could use this to track throughput in MB/s
        // but it would require byte-based ProgressBar
    }

    fn on_phase_end(&self, phase: &str) {
        if self.quiet {
            return;
        }

        match phase {
            "walking" => {
                if let Some(pb) = self.walking.lock().unwrap().take() {
                    pb.finish_with_message("Walking complete");
                }
            }
            "prehash" => {
                if let Some(pb) = self.prehash.lock().unwrap().take() {
                    pb.finish_with_message("Prehashing complete");
                }
            }
            "fullhash" => {
                if let Some(pb) = self.fullhash.lock().unwrap().take() {
                    pb.finish_with_message("Full hashing complete");
                }
            }
            _ => {}
        }
    }
}

/// Truncate a path for display in the progress bar.
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }

    let path_buf = std::path::Path::new(path);
    let file_name = path_buf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if file_name.len() >= max_len {
        return format!("...{}", &file_name[file_name.len() - max_len + 3..]);
    }

    format!(".../{}", file_name)
}
