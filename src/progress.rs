//! Progress reporting utilities using indicatif.
//!
//! This module provides the [`Progress`] struct which implements [`ProgressCallback`]
//! to display visual progress bars in the terminal for non-TUI output modes.
//!
//! # Accessible Mode
//!
//! When accessible mode is enabled, progress reporting uses simplified output:
//! - No spinners or animations
//! - Plain text updates without cursor movement
//! - Reduced update frequency for screen reader compatibility

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use indicatif::{HumanBytes, HumanDuration, MultiProgress, ProgressBar, ProgressStyle};

/// State for exponential moving average metrics (ETA and throughput).
#[derive(Debug)]
struct ProgressMetrics {
    ema_rate: f64,      // items/sec
    ema_byte_rate: f64, // bytes/sec
    last_pos: u64,
    last_bytes: u64,
    last_update: Instant,
    initialized: bool,
    alpha: f64,
}

impl ProgressMetrics {
    fn new() -> Self {
        Self {
            ema_rate: 0.0,
            ema_byte_rate: 0.0,
            last_pos: 0,
            last_bytes: 0,
            last_update: Instant::now(),
            initialized: false,
            alpha: 0.1, // Smoothing factor (0.1 = more stable, 0.2 = more responsive)
        }
    }

    fn update(&mut self, pos: u64, bytes: u64) {
        let now = Instant::now();
        let delta_t = now.duration_since(self.last_update).as_secs_f64();
        let delta_pos = pos.saturating_sub(self.last_pos);
        let delta_bytes = bytes.saturating_sub(self.last_bytes);

        // Update at most every 100ms
        if delta_t < 0.1 {
            return;
        }

        if delta_pos > 0 {
            let instant_rate = delta_pos as f64 / delta_t;
            if !self.initialized {
                self.ema_rate = instant_rate;
            } else {
                self.ema_rate = self.alpha * instant_rate + (1.0 - self.alpha) * self.ema_rate;
            }
        }

        if delta_bytes > 0 {
            let instant_byte_rate = delta_bytes as f64 / delta_t;
            if !self.initialized {
                self.ema_byte_rate = instant_byte_rate;
            } else {
                self.ema_byte_rate =
                    self.alpha * instant_byte_rate + (1.0 - self.alpha) * self.ema_byte_rate;
            }
        }

        if !self.initialized && (delta_pos > 0 || delta_bytes > 0) {
            self.initialized = true;
        }

        self.last_pos = pos;
        self.last_bytes = bytes;
        self.last_update = now;
    }

    fn estimate_eta(&self, remaining: u64) -> Option<Duration> {
        if !self.initialized || self.ema_rate <= 0.0 {
            return None;
        }

        let secs = remaining as f64 / self.ema_rate;
        // Don't show ETA if it's unreasonably large (e.g., > 1 week)
        if secs > 3600.0 * 24.0 * 7.0 {
            return None;
        }

        Some(Duration::from_secs_f64(secs))
    }

    fn rate_items(&self) -> f64 {
        self.ema_rate
    }

    fn rate_bytes(&self) -> f64 {
        self.ema_byte_rate
    }
}

/// Progress callback for duplicate finding phases.
///
/// Implement this trait to receive progress updates during
/// the duplicate detection pipeline.
pub trait ProgressCallback: Send + Sync {
    /// Called when a phase starts.
    ///
    /// # Arguments
    ///
    /// * `phase` - Name of the phase (e.g., "prehash", "fullhash")
    /// * `total` - Total number of items to process
    fn on_phase_start(&self, phase: &str, total: usize);

    /// Called for each item processed.
    ///
    /// # Arguments
    ///
    /// * `current` - Current item number (1-based)
    /// * `path` - Path being processed
    fn on_progress(&self, current: usize, path: &str);

    /// Called when an item has been processed, providing its size.
    ///
    /// This can be used to track byte-based throughput.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Size of the item in bytes
    fn on_item_completed(&self, _bytes: u64) {}

    /// Called when a phase completes.
    ///
    /// # Arguments
    ///
    /// * `phase` - Name of the phase
    fn on_phase_end(&self, phase: &str);

    /// Called to update the progress message.
    ///
    /// # Arguments
    ///
    /// * `message` - The new message to display
    fn on_message(&self, _message: &str) {}
}

/// Progress reporter using indicatif.
///
/// Manages multiple progress bars for different phases of the duplicate
/// detection pipeline.
pub struct Progress {
    multi: MultiProgress,
    walking: Mutex<Option<ProgressBar>>,
    prehash: Mutex<Option<ProgressBar>>,
    fullhash: Mutex<Option<ProgressBar>>,
    prefix: Mutex<String>,
    phase_name: Mutex<String>,
    active_phase: Mutex<Option<String>>,
    metrics: Mutex<HashMap<String, ProgressMetrics>>,
    total_bytes: Mutex<HashMap<String, u64>>,
    quiet: bool,
    accessible: bool,
}

impl Progress {
    /// Create a new progress reporter.
    ///
    /// # Arguments
    ///
    /// * `quiet` - If true, no progress bars will be displayed.
    /// # Examples
    ///
    /// ```
    /// use rustdupe::progress::Progress;
    ///
    /// let progress = Progress::new(false);
    /// ```
    #[must_use]
    pub fn new(quiet: bool) -> Self {
        Self {
            multi: MultiProgress::new(),
            walking: Mutex::new(None),
            prehash: Mutex::new(None),
            fullhash: Mutex::new(None),
            prefix: Mutex::new(String::new()),
            phase_name: Mutex::new(String::new()),
            active_phase: Mutex::new(None),
            metrics: Mutex::new(HashMap::new()),
            total_bytes: Mutex::new(HashMap::new()),
            quiet,
            accessible: false,
        }
    }

    /// Create a new progress reporter with accessible mode.
    ///
    /// # Arguments
    ///
    /// * `quiet` - If true, no progress will be displayed.
    /// * `accessible` - If true, uses simplified output for screen readers.
    ///
    /// # Examples
    ///
    /// ```
    /// use rustdupe::progress::Progress;
    ///
    /// let progress = Progress::with_accessible(false, true);
    /// ```
    #[must_use]
    pub fn with_accessible(quiet: bool, accessible: bool) -> Self {
        Self {
            multi: MultiProgress::new(),
            walking: Mutex::new(None),
            prehash: Mutex::new(None),
            fullhash: Mutex::new(None),
            prefix: Mutex::new(String::new()),
            phase_name: Mutex::new(String::new()),
            active_phase: Mutex::new(None),
            metrics: Mutex::new(HashMap::new()),
            total_bytes: Mutex::new(HashMap::new()),
            quiet,
            accessible,
        }
    }

    /// Check if accessible mode is enabled.
    #[must_use]
    pub fn is_accessible(&self) -> bool {
        self.accessible
    }

    /// Create a style for the walking phase (spinner).
    fn walking_style(&self) -> ProgressStyle {
        if self.accessible {
            // Accessible: No spinner animation, just text
            ProgressStyle::with_template("{msg} [{elapsed_precise}] {pos} files")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
        } else {
            ProgressStyle::with_template("{spinner:.green} {msg} [{elapsed_precise}] {pos} files")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        }
    }

    /// Create a style for the prehash phase (progress bar).
    fn prehash_style(&self) -> ProgressStyle {
        if self.accessible {
            // Accessible: ASCII progress bar, no Unicode
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40}] {pos}/{len} ({percent}%) {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("#>-")
        } else {
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█>-")
        }
    }

    /// Create a style for the fullhash phase (progress bar with throughput).
    fn fullhash_style(&self) -> ProgressStyle {
        if self.accessible {
            // Accessible: ASCII progress bar, no Unicode
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40}] {pos}/{len} ({percent}%) {msg} {per_sec}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("#>-")
        } else {
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.green/blue}] {pos}/{len} ({percent}%) {msg} {per_sec}",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█>-")
        }
    }
}

impl ProgressCallback for Progress {
    fn on_phase_start(&self, phase: &str, total: usize) {
        if self.quiet {
            return;
        }

        *self.active_phase.lock().unwrap() = Some(phase.to_string());
        let display_name = match phase {
            "walking" => "Walking",
            "prehash" => "Prehashing",
            "fullhash" => "Full Hashing",
            "perceptual_hashing" => "Perceptual Hashing",
            _ => {
                let name = phase.replace('_', " ");
                let mut chars = name.chars();
                match chars.next() {
                    None => "",
                    Some(f) => {
                        let capitalized = f.to_uppercase().collect::<String>() + chars.as_str();
                        Box::leak(capitalized.into_boxed_str())
                    }
                }
            }
        };
        *self.phase_name.lock().unwrap() = display_name.to_string();
        self.metrics
            .lock()
            .unwrap()
            .insert(phase.to_string(), ProgressMetrics::new());
        self.total_bytes
            .lock()
            .unwrap()
            .insert(phase.to_string(), 0);

        match phase {
            "walking" => {
                let pb = self.multi.add(ProgressBar::new_spinner());
                pb.set_style(self.walking_style());
                pb.set_message("Walking");
                // In accessible mode, use a slower tick rate
                let tick_rate = if self.accessible { 500 } else { 100 };
                pb.enable_steady_tick(Duration::from_millis(tick_rate));
                let mut walking = self.walking.lock().unwrap();
                *walking = Some(pb);
            }
            "prehash" => {
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(self.prehash_style());
                pb.set_message("Prehashing");
                let mut prehash = self.prehash.lock().unwrap();
                *prehash = Some(pb);
            }
            "fullhash" => {
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(self.fullhash_style());
                pb.set_message("Full Hashing");
                let mut fullhash = self.fullhash.lock().unwrap();
                *fullhash = Some(pb);
            }
            "perceptual_hashing" => {
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(self.prehash_style());
                pb.set_message("Perceptual Hashing");
                let mut prehash = self.prehash.lock().unwrap();
                *prehash = Some(pb);
            }
            _ => {
                // For any other phase, use a default bar
                let pb = self.multi.add(ProgressBar::new(total as u64));
                pb.set_style(self.prehash_style());
                let name = phase.replace('_', " ");
                let mut chars = name.chars();
                let capitalized = match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                };
                pb.set_message(capitalized);
                let mut prehash = self.prehash.lock().unwrap();
                *prehash = Some(pb);
            }
        }
    }

    fn on_progress(&self, current: usize, path: &str) {
        if self.quiet {
            return;
        }

        let prefix = self.prefix.lock().unwrap();
        let phase_name = self.phase_name.lock().unwrap();
        let display_msg = if prefix.is_empty() {
            format!("{}: {}", *phase_name, truncate_path(path, 30))
        } else {
            format!("{}: {}: {}", *phase_name, *prefix, truncate_path(path, 30))
        };

        // Update metrics and generate status
        let mut metrics_display = String::new();
        let active_phase = self.active_phase.lock().unwrap();
        if let Some(ref phase) = *active_phase {
            let bytes = self
                .total_bytes
                .lock()
                .unwrap()
                .get(phase)
                .copied()
                .unwrap_or(0);

            let mut metrics_map = self.metrics.lock().unwrap();
            if let Some(m) = metrics_map.get_mut(phase) {
                m.update(current as u64, bytes);

                // Throughput
                let items_rate = m.rate_items();
                let bytes_rate = m.rate_bytes();

                if items_rate > 0.0 {
                    metrics_display = format!(" ({:.1} files/s", items_rate);
                    if bytes_rate > 0.0 {
                        metrics_display.push_str(&format!(", {}/s", HumanBytes(bytes_rate as u64)));
                    }

                    // ETA
                    // Only show ETA if we have a total length
                    let total = match phase.as_str() {
                        "fullhash" => self
                            .fullhash
                            .lock()
                            .unwrap()
                            .as_ref()
                            .and_then(|pb| pb.length()),
                        "prehash" => self
                            .prehash
                            .lock()
                            .unwrap()
                            .as_ref()
                            .and_then(|pb| pb.length()),
                        _ => None,
                    }
                    .unwrap_or(0);

                    if total > 0 {
                        let remaining = total.saturating_sub(current as u64);
                        if let Some(est) = m.estimate_eta(remaining) {
                            metrics_display.push_str(&format!(" | ETA: {}", HumanDuration(est)));
                        }
                    }
                    metrics_display.push(')');
                }
            }
        }

        let final_msg = format!("{}{}", display_msg, metrics_display);

        // Update the active progress bar
        if let Some(ref pb) = *self.fullhash.lock().unwrap() {
            pb.set_position(current as u64);
            pb.set_message(final_msg);
        } else if let Some(ref pb) = *self.prehash.lock().unwrap() {
            pb.set_position(current as u64);
            pb.set_message(final_msg);
        } else if let Some(ref pb) = *self.walking.lock().unwrap() {
            pb.set_position(current as u64);
            pb.set_message(final_msg);
        }
    }

    fn on_item_completed(&self, bytes: u64) {
        if self.quiet {
            return;
        }

        let active_phase = self.active_phase.lock().unwrap();
        if let Some(ref phase) = *active_phase {
            let mut total_bytes = self.total_bytes.lock().unwrap();
            if let Some(entry) = total_bytes.get_mut(phase) {
                *entry += bytes;
            }
        }
    }

    fn on_phase_end(&self, phase: &str) {
        if self.quiet {
            return;
        }

        *self.active_phase.lock().unwrap() = None;

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

    fn on_message(&self, message: &str) {
        if self.quiet {
            return;
        }

        *self.prefix.lock().unwrap() = message.to_string();

        if let Some(ref pb) = *self.fullhash.lock().unwrap() {
            pb.set_message(message.to_string());
        } else if let Some(ref pb) = *self.prehash.lock().unwrap() {
            pb.set_message(message.to_string());
        } else if let Some(ref pb) = *self.walking.lock().unwrap() {
            pb.set_message(message.to_string());
        }
    }
}

/// Truncate a path for display in the progress bar.
fn truncate_path(path: &str, max_len: usize) -> String {
    // Use character count, not byte count, for proper Unicode handling
    let char_count = path.chars().count();
    if char_count <= max_len {
        return path.to_string();
    }

    let path_buf = std::path::Path::new(path);
    let file_name = path_buf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let file_name_chars = file_name.chars().count();
    if file_name_chars >= max_len {
        // Truncate filename from the start, keeping the end
        let skip_chars = file_name_chars.saturating_sub(max_len.saturating_sub(3));
        let truncated: String = file_name.chars().skip(skip_chars).collect();
        return format!("...{}", truncated);
    }

    format!(".../{}", file_name)
}
