//! TUI application state management.
//!
//! # Overview
//!
//! This module manages the application state for the interactive TUI, including:
//! - Current mode (Scanning, Reviewing, Previewing, Confirming, Quitting)
//! - Duplicate groups for display
//! - Navigation state (selected index, scroll offset)
//! - Selection state (files marked for deletion)
//!
//! # Architecture
//!
//! The `App` struct is the central state container for the TUI. It is designed
//! to be accessed only from the main thread (terminal operations are not thread-safe).
//! State transitions are explicit through method calls.
//!
//! # Example
//!
//! ```
//! use rustdupe::tui::app::{App, AppMode};
//! use rustdupe::duplicates::DuplicateGroup;
//! use std::path::PathBuf;
//!
//! // Create a new app instance
//! let mut app = App::new();
//!
//! // Set up with duplicate groups
//! let groups = vec![
//!     DuplicateGroup::new(
//!         [0u8; 32],
//!         1000,
//!         vec![PathBuf::from("/a.txt"), PathBuf::from("/b.txt")],
//!     ),
//! ];
//! app.set_groups(groups);
//! app.set_mode(AppMode::Reviewing);
//!
//! // Navigate and select
//! app.next();
//! app.toggle_select();
//!
//! assert!(app.is_file_selected(&PathBuf::from("/b.txt")));
//! ```

use std::collections::HashSet;
use std::path::PathBuf;

use crate::duplicates::DuplicateGroup;

/// Application mode/state.
///
/// Represents the current state of the TUI application. Modes control
/// what is displayed and which actions are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Scanning in progress - shows progress bar and stats
    #[default]
    Scanning,
    /// Reviewing duplicate groups - main navigation mode
    Reviewing,
    /// Previewing a file's content
    Previewing,
    /// Confirming a deletion operation
    Confirming,
    /// Application is quitting
    Quitting,
}

impl AppMode {
    /// Check if the application is in a navigable state.
    #[must_use]
    pub fn is_navigable(&self) -> bool {
        matches!(self, Self::Reviewing)
    }

    /// Check if the application is done (quitting).
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Quitting)
    }
}

/// User action triggered by keyboard input.
///
/// Actions are the result of key event processing and represent
/// user intentions that modify application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Navigate up in the list
    NavigateUp,
    /// Navigate down in the list
    NavigateDown,
    /// Navigate to next group
    NextGroup,
    /// Navigate to previous group
    PreviousGroup,
    /// Toggle selection of current item
    ToggleSelect,
    /// Select all files in current group (except first)
    SelectAllInGroup,
    /// Deselect all files
    DeselectAll,
    /// Preview the selected file
    Preview,
    /// Delete selected files (to trash)
    Delete,
    /// Confirm current action
    Confirm,
    /// Cancel current action
    Cancel,
    /// Quit the application
    Quit,
}

/// Scan summary for display in TUI.
///
/// Contains statistics about the completed scan to display to the user.
#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    /// Current phase name (e.g., "Walking", "Prehashing", "Full hashing")
    pub phase: String,
    /// Current file being processed
    pub current_path: String,
    /// Number of files processed so far
    pub current: usize,
    /// Total number of files to process (0 if unknown)
    pub total: usize,
    /// Human-readable status message
    pub message: String,
}

impl ScanProgress {
    /// Create a new scan progress.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate progress percentage (0-100).
    #[must_use]
    pub fn percentage(&self) -> u16 {
        if self.total == 0 {
            0
        } else {
            ((self.current as f64 / self.total as f64) * 100.0).min(100.0) as u16
        }
    }
}

/// TUI application state.
///
/// The central state container for the TUI application. Manages:
/// - Current mode and navigation state
/// - Duplicate groups to display
/// - User selections for batch operations
///
/// # Thread Safety
///
/// This struct is NOT thread-safe and should only be accessed from the main thread.
/// Terminal operations are not thread-safe, so all TUI state modifications
/// must happen on the main thread.
#[derive(Debug, Clone)]
pub struct App {
    /// Current application mode
    mode: AppMode,
    /// Duplicate groups to display
    groups: Vec<DuplicateGroup>,
    /// Currently selected group index
    group_index: usize,
    /// Currently selected file index within the group
    file_index: usize,
    /// Scroll offset for the group list
    group_scroll: usize,
    /// Scroll offset for the file list within current group
    file_scroll: usize,
    /// Files marked for deletion (PathBuf set for O(1) lookup)
    selected_files: HashSet<PathBuf>,
    /// Scan progress (for Scanning mode)
    scan_progress: ScanProgress,
    /// Error message to display (if any)
    error_message: Option<String>,
    /// Preview content (for Previewing mode)
    preview_content: Option<String>,
    /// Total reclaimable space in bytes
    reclaimable_space: u64,
    /// Number of visible rows in the UI (for scroll calculation)
    visible_rows: usize,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new App instance with empty state.
    ///
    /// The app starts in Scanning mode with no groups loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: AppMode::Scanning,
            groups: Vec::new(),
            group_index: 0,
            file_index: 0,
            group_scroll: 0,
            file_scroll: 0,
            selected_files: HashSet::new(),
            scan_progress: ScanProgress::new(),
            error_message: None,
            preview_content: None,
            reclaimable_space: 0,
            visible_rows: 20, // Default, will be updated by UI
        }
    }

    /// Create an App with pre-loaded duplicate groups.
    ///
    /// The app starts in Reviewing mode if groups are provided.
    #[must_use]
    pub fn with_groups(groups: Vec<DuplicateGroup>) -> Self {
        let reclaimable = groups.iter().map(DuplicateGroup::wasted_space).sum();
        let mode = if groups.is_empty() {
            AppMode::Scanning
        } else {
            AppMode::Reviewing
        };

        Self {
            mode,
            groups,
            group_index: 0,
            file_index: 0,
            group_scroll: 0,
            file_scroll: 0,
            selected_files: HashSet::new(),
            scan_progress: ScanProgress::new(),
            error_message: None,
            preview_content: None,
            reclaimable_space: reclaimable,
            visible_rows: 20,
        }
    }

    // ==================== Mode Management ====================

    /// Get the current application mode.
    #[must_use]
    pub fn mode(&self) -> AppMode {
        self.mode
    }

    /// Set the application mode.
    ///
    /// This is the only way to change modes - state transitions are explicit.
    pub fn set_mode(&mut self, mode: AppMode) {
        log::debug!("Mode transition: {:?} -> {:?}", self.mode, mode);
        self.mode = mode;
    }

    /// Check if the application should quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.mode.is_done()
    }

    // ==================== Group Management ====================

    /// Get the duplicate groups.
    #[must_use]
    pub fn groups(&self) -> &[DuplicateGroup] {
        &self.groups
    }

    /// Set the duplicate groups and recalculate stats.
    ///
    /// This also resets navigation state and calculates reclaimable space.
    pub fn set_groups(&mut self, groups: Vec<DuplicateGroup>) {
        self.reclaimable_space = groups.iter().map(DuplicateGroup::wasted_space).sum();
        self.groups = groups;
        self.group_index = 0;
        self.file_index = 0;
        self.group_scroll = 0;
        self.file_scroll = 0;
        self.selected_files.clear();

        log::info!(
            "Loaded {} duplicate groups, {} bytes reclaimable",
            self.groups.len(),
            self.reclaimable_space
        );
    }

    /// Get the number of duplicate groups.
    #[must_use]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Check if there are any duplicate groups.
    #[must_use]
    pub fn has_groups(&self) -> bool {
        !self.groups.is_empty()
    }

    /// Get the total reclaimable space in bytes.
    #[must_use]
    pub fn reclaimable_space(&self) -> u64 {
        self.reclaimable_space
    }

    /// Get the total number of duplicate files.
    #[must_use]
    pub fn duplicate_file_count(&self) -> usize {
        self.groups.iter().map(|g| g.files.len()).sum()
    }

    // ==================== Navigation ====================

    /// Get the currently selected group index.
    #[must_use]
    pub fn group_index(&self) -> usize {
        self.group_index
    }

    /// Get the currently selected file index within the group.
    #[must_use]
    pub fn file_index(&self) -> usize {
        self.file_index
    }

    /// Get the current group scroll offset.
    #[must_use]
    pub fn group_scroll(&self) -> usize {
        self.group_scroll
    }

    /// Get the current file scroll offset.
    #[must_use]
    pub fn file_scroll(&self) -> usize {
        self.file_scroll
    }

    /// Set the number of visible rows (for scroll calculation).
    pub fn set_visible_rows(&mut self, rows: usize) {
        self.visible_rows = rows.max(1);
    }

    /// Get the currently selected group (if any).
    #[must_use]
    pub fn current_group(&self) -> Option<&DuplicateGroup> {
        self.groups.get(self.group_index)
    }

    /// Get the currently selected file path (if any).
    #[must_use]
    pub fn current_file(&self) -> Option<&PathBuf> {
        self.current_group()
            .and_then(|g| g.files.get(self.file_index))
    }

    /// Navigate to the next file in the current group.
    ///
    /// If at the end of the group, stays at the last file.
    pub fn next(&mut self) {
        if !self.mode.is_navigable() || self.groups.is_empty() {
            return;
        }

        if let Some(group) = self.current_group() {
            if self.file_index + 1 < group.files.len() {
                self.file_index += 1;
                self.update_file_scroll();
                log::trace!("Navigate next: file_index = {}", self.file_index);
            }
        }
    }

    /// Navigate to the previous file in the current group.
    ///
    /// If at the start of the group, stays at the first file.
    pub fn previous(&mut self) {
        if !self.mode.is_navigable() || self.groups.is_empty() {
            return;
        }

        if self.file_index > 0 {
            self.file_index -= 1;
            self.update_file_scroll();
            log::trace!("Navigate previous: file_index = {}", self.file_index);
        }
    }

    /// Navigate to the next duplicate group.
    pub fn next_group(&mut self) {
        if !self.mode.is_navigable() || self.groups.is_empty() {
            return;
        }

        if self.group_index + 1 < self.groups.len() {
            self.group_index += 1;
            self.file_index = 0;
            self.file_scroll = 0;
            self.update_group_scroll();
            log::trace!("Navigate next group: group_index = {}", self.group_index);
        }
    }

    /// Navigate to the previous duplicate group.
    pub fn previous_group(&mut self) {
        if !self.mode.is_navigable() || self.groups.is_empty() {
            return;
        }

        if self.group_index > 0 {
            self.group_index -= 1;
            self.file_index = 0;
            self.file_scroll = 0;
            self.update_group_scroll();
            log::trace!(
                "Navigate previous group: group_index = {}",
                self.group_index
            );
        }
    }

    /// Update file scroll to keep current selection visible.
    fn update_file_scroll(&mut self) {
        // Scroll down if selection is below visible area
        if self.file_index >= self.file_scroll + self.visible_rows {
            self.file_scroll = self.file_index - self.visible_rows + 1;
        }
        // Scroll up if selection is above visible area
        if self.file_index < self.file_scroll {
            self.file_scroll = self.file_index;
        }
    }

    /// Update group scroll to keep current selection visible.
    fn update_group_scroll(&mut self) {
        // Scroll down if selection is below visible area
        if self.group_index >= self.group_scroll + self.visible_rows {
            self.group_scroll = self.group_index - self.visible_rows + 1;
        }
        // Scroll up if selection is above visible area
        if self.group_index < self.group_scroll {
            self.group_scroll = self.group_index;
        }
    }

    // ==================== Selection Management ====================

    /// Get the set of selected file paths.
    #[must_use]
    pub fn selected_files(&self) -> &HashSet<PathBuf> {
        &self.selected_files
    }

    /// Get selected files as a sorted vector (for display/operations).
    #[must_use]
    pub fn selected_files_vec(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self.selected_files.iter().cloned().collect();
        files.sort();
        files
    }

    /// Get the number of selected files.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selected_files.len()
    }

    /// Check if any files are selected.
    #[must_use]
    pub fn has_selections(&self) -> bool {
        !self.selected_files.is_empty()
    }

    /// Check if a specific file is selected.
    #[must_use]
    pub fn is_file_selected(&self, path: &PathBuf) -> bool {
        self.selected_files.contains(path)
    }

    /// Check if the currently highlighted file is selected.
    #[must_use]
    pub fn is_current_selected(&self) -> bool {
        self.current_file()
            .is_some_and(|f| self.selected_files.contains(f))
    }

    /// Toggle selection of the currently highlighted file.
    ///
    /// If the file is selected, it will be deselected, and vice versa.
    pub fn toggle_select(&mut self) {
        if let Some(path) = self.current_file().cloned() {
            if self.selected_files.contains(&path) {
                self.selected_files.remove(&path);
                log::debug!("Deselected: {}", path.display());
            } else {
                self.selected_files.insert(path.clone());
                log::debug!("Selected: {}", path.display());
            }
        }
    }

    /// Select a specific file.
    pub fn select(&mut self, path: PathBuf) {
        self.selected_files.insert(path);
    }

    /// Deselect a specific file.
    pub fn deselect(&mut self, path: &PathBuf) {
        self.selected_files.remove(path);
    }

    /// Select all files in the current group except the first one.
    ///
    /// The first file is preserved as the "original" that should be kept.
    pub fn select_all_in_group(&mut self) {
        // Clone files to avoid borrow conflict
        let files_to_select: Vec<PathBuf> = self
            .current_group()
            .map(|g| g.files.iter().skip(1).cloned().collect())
            .unwrap_or_default();

        let count = files_to_select.len();
        for path in files_to_select {
            self.selected_files.insert(path);
        }

        if count > 0 {
            log::debug!("Selected {} files in group (keeping first)", count);
        }
    }

    /// Deselect all files.
    pub fn deselect_all(&mut self) {
        let count = self.selected_files.len();
        self.selected_files.clear();
        log::debug!("Deselected all {} files", count);
    }

    /// Remove files from groups after successful deletion.
    ///
    /// This updates the internal state to reflect deleted files.
    pub fn remove_deleted_files(&mut self, deleted: &[PathBuf]) {
        let deleted_set: HashSet<&PathBuf> = deleted.iter().collect();

        // Remove from selection
        self.selected_files.retain(|p| !deleted_set.contains(p));

        // Remove from groups and filter empty groups
        for group in &mut self.groups {
            group.files.retain(|p| !deleted_set.contains(p));
        }

        // Remove groups with less than 2 files (no longer duplicates)
        self.groups.retain(|g| g.files.len() >= 2);

        // Recalculate reclaimable space
        self.reclaimable_space = self.groups.iter().map(DuplicateGroup::wasted_space).sum();

        // Fix navigation if needed
        if self.group_index >= self.groups.len() && !self.groups.is_empty() {
            self.group_index = self.groups.len() - 1;
        }
        if let Some(group) = self.current_group() {
            if self.file_index >= group.files.len() && !group.files.is_empty() {
                self.file_index = group.files.len() - 1;
            }
        } else {
            self.file_index = 0;
        }

        log::info!(
            "Removed {} deleted files, {} groups remaining",
            deleted.len(),
            self.groups.len()
        );
    }

    // ==================== Scan Progress ====================

    /// Get the scan progress.
    #[must_use]
    pub fn scan_progress(&self) -> &ScanProgress {
        &self.scan_progress
    }

    /// Update the scan progress.
    pub fn update_scan_progress(&mut self, phase: &str, current: usize, total: usize, path: &str) {
        self.scan_progress.phase = phase.to_string();
        self.scan_progress.current = current;
        self.scan_progress.total = total;
        self.scan_progress.current_path = path.to_string();
    }

    /// Set a status message for the scan progress.
    pub fn set_scan_message(&mut self, message: &str) {
        self.scan_progress.message = message.to_string();
    }

    // ==================== Error Handling ====================

    /// Get the current error message (if any).
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Set an error message to display.
    pub fn set_error(&mut self, message: &str) {
        self.error_message = Some(message.to_string());
        log::error!("App error: {}", message);
    }

    /// Clear the error message.
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    // ==================== Preview ====================

    /// Get the preview content (if any).
    #[must_use]
    pub fn preview_content(&self) -> Option<&str> {
        self.preview_content.as_deref()
    }

    /// Set the preview content.
    pub fn set_preview(&mut self, content: String) {
        self.preview_content = Some(content);
    }

    /// Clear the preview content.
    pub fn clear_preview(&mut self) {
        self.preview_content = None;
    }

    // ==================== Action Handling ====================

    /// Handle a user action and update state accordingly.
    ///
    /// Returns true if the action was handled.
    pub fn handle_action(&mut self, action: Action) -> bool {
        log::trace!("Handling action: {:?} in mode {:?}", action, self.mode);

        match action {
            Action::NavigateUp => {
                self.previous();
                true
            }
            Action::NavigateDown => {
                self.next();
                true
            }
            Action::NextGroup => {
                self.next_group();
                true
            }
            Action::PreviousGroup => {
                self.previous_group();
                true
            }
            Action::ToggleSelect => {
                self.toggle_select();
                true
            }
            Action::SelectAllInGroup => {
                self.select_all_in_group();
                true
            }
            Action::DeselectAll => {
                self.deselect_all();
                true
            }
            Action::Preview => {
                if self.mode == AppMode::Reviewing && self.current_file().is_some() {
                    self.set_mode(AppMode::Previewing);
                    true
                } else {
                    false
                }
            }
            Action::Delete => {
                if self.mode == AppMode::Reviewing && self.has_selections() {
                    self.set_mode(AppMode::Confirming);
                    true
                } else {
                    false
                }
            }
            Action::Confirm => {
                // Confirmation handling is done by the TUI main loop
                true
            }
            Action::Cancel => {
                match self.mode {
                    AppMode::Previewing => {
                        self.clear_preview();
                        self.set_mode(AppMode::Reviewing);
                    }
                    AppMode::Confirming => {
                        self.set_mode(AppMode::Reviewing);
                    }
                    _ => {}
                }
                true
            }
            Action::Quit => {
                self.set_mode(AppMode::Quitting);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_group(size: u64, paths: Vec<&str>) -> DuplicateGroup {
        DuplicateGroup::new(
            [0u8; 32],
            size,
            paths.into_iter().map(PathBuf::from).collect(),
        )
    }

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert_eq!(app.mode(), AppMode::Scanning);
        assert!(app.groups().is_empty());
        assert_eq!(app.group_index(), 0);
        assert_eq!(app.file_index(), 0);
        assert!(!app.has_selections());
    }

    #[test]
    fn test_app_with_groups() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let app = App::with_groups(groups);

        assert_eq!(app.mode(), AppMode::Reviewing);
        assert_eq!(app.group_count(), 1);
        assert_eq!(app.reclaimable_space(), 100); // 1 duplicate = 100 bytes wasted
    }

    #[test]
    fn test_app_with_empty_groups() {
        let app = App::with_groups(vec![]);
        assert_eq!(app.mode(), AppMode::Scanning);
        assert!(!app.has_groups());
    }

    #[test]
    fn test_set_groups() {
        let mut app = App::new();
        let groups = vec![
            make_group(100, vec!["/a.txt", "/b.txt"]),
            make_group(200, vec!["/c.txt", "/d.txt", "/e.txt"]),
        ];
        app.set_groups(groups);

        assert_eq!(app.group_count(), 2);
        assert_eq!(app.reclaimable_space(), 100 + 400); // 1*100 + 2*200
        assert_eq!(app.group_index(), 0);
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_navigation_next_previous() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt", "/c.txt"])];
        let mut app = App::with_groups(groups);

        assert_eq!(app.file_index(), 0);

        app.next();
        assert_eq!(app.file_index(), 1);

        app.next();
        assert_eq!(app.file_index(), 2);

        // At end, should stay at last
        app.next();
        assert_eq!(app.file_index(), 2);

        app.previous();
        assert_eq!(app.file_index(), 1);

        app.previous();
        assert_eq!(app.file_index(), 0);

        // At start, should stay at first
        app.previous();
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_navigation_groups() {
        let groups = vec![
            make_group(100, vec!["/a.txt", "/b.txt"]),
            make_group(200, vec!["/c.txt", "/d.txt"]),
            make_group(300, vec!["/e.txt", "/f.txt"]),
        ];
        let mut app = App::with_groups(groups);

        assert_eq!(app.group_index(), 0);

        app.next_group();
        assert_eq!(app.group_index(), 1);
        assert_eq!(app.file_index(), 0); // Reset file index

        app.next_group();
        assert_eq!(app.group_index(), 2);

        // At end, should stay
        app.next_group();
        assert_eq!(app.group_index(), 2);

        app.previous_group();
        assert_eq!(app.group_index(), 1);

        app.previous_group();
        assert_eq!(app.group_index(), 0);

        // At start, should stay
        app.previous_group();
        assert_eq!(app.group_index(), 0);
    }

    #[test]
    fn test_navigation_not_in_reviewing_mode() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);
        app.set_mode(AppMode::Scanning);

        app.next();
        assert_eq!(app.file_index(), 0); // Should not move
    }

    #[test]
    fn test_toggle_select() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        assert!(!app.is_current_selected());

        app.toggle_select();
        assert!(app.is_current_selected());
        assert_eq!(app.selected_count(), 1);

        app.toggle_select();
        assert!(!app.is_current_selected());
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn test_select_all_in_group() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt", "/c.txt"])];
        let mut app = App::with_groups(groups);

        app.select_all_in_group();

        // First file should NOT be selected (preserved as original)
        assert!(!app.is_file_selected(&PathBuf::from("/a.txt")));
        assert!(app.is_file_selected(&PathBuf::from("/b.txt")));
        assert!(app.is_file_selected(&PathBuf::from("/c.txt")));
        assert_eq!(app.selected_count(), 2);
    }

    #[test]
    fn test_deselect_all() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt", "/c.txt"])];
        let mut app = App::with_groups(groups);

        app.select_all_in_group();
        assert_eq!(app.selected_count(), 2);

        app.deselect_all();
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn test_selected_files_vec() {
        let groups = vec![make_group(100, vec!["/z.txt", "/a.txt", "/m.txt"])];
        let mut app = App::with_groups(groups);

        app.select_all_in_group();
        let selected = app.selected_files_vec();

        // Should be sorted
        assert_eq!(
            selected,
            vec![PathBuf::from("/a.txt"), PathBuf::from("/m.txt")]
        );
    }

    #[test]
    fn test_remove_deleted_files() {
        let groups = vec![
            make_group(100, vec!["/a.txt", "/b.txt", "/c.txt"]),
            make_group(200, vec!["/d.txt", "/e.txt"]),
        ];
        let mut app = App::with_groups(groups);
        app.select(PathBuf::from("/b.txt"));
        app.select(PathBuf::from("/e.txt"));

        // Delete /b.txt and /e.txt
        app.remove_deleted_files(&[PathBuf::from("/b.txt"), PathBuf::from("/e.txt")]);

        // /b.txt should be removed from first group
        assert_eq!(app.groups()[0].files.len(), 2);
        assert!(!app.groups()[0].files.contains(&PathBuf::from("/b.txt")));

        // Second group now has only 1 file, so it's removed (not duplicates anymore)
        assert_eq!(app.group_count(), 1);

        // Selections should be cleared for deleted files
        assert!(!app.is_file_selected(&PathBuf::from("/b.txt")));
        assert!(!app.is_file_selected(&PathBuf::from("/e.txt")));
    }

    #[test]
    fn test_current_file() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        assert_eq!(app.current_file(), Some(&PathBuf::from("/a.txt")));

        app.next();
        assert_eq!(app.current_file(), Some(&PathBuf::from("/b.txt")));
    }

    #[test]
    fn test_current_group() {
        let groups = vec![
            make_group(100, vec!["/a.txt", "/b.txt"]),
            make_group(200, vec!["/c.txt", "/d.txt"]),
        ];
        let mut app = App::with_groups(groups);

        let group = app.current_group().unwrap();
        assert_eq!(group.size, 100);

        app.next_group();
        let group = app.current_group().unwrap();
        assert_eq!(group.size, 200);
    }

    #[test]
    fn test_mode_transitions() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        assert_eq!(app.mode(), AppMode::Reviewing);

        app.set_mode(AppMode::Previewing);
        assert_eq!(app.mode(), AppMode::Previewing);

        app.set_mode(AppMode::Confirming);
        assert_eq!(app.mode(), AppMode::Confirming);

        app.set_mode(AppMode::Quitting);
        assert!(app.should_quit());
    }

    #[test]
    fn test_handle_action_navigate() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt", "/c.txt"])];
        let mut app = App::with_groups(groups);

        assert!(app.handle_action(Action::NavigateDown));
        assert_eq!(app.file_index(), 1);

        assert!(app.handle_action(Action::NavigateUp));
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_handle_action_toggle_select() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        assert!(app.handle_action(Action::ToggleSelect));
        assert!(app.is_current_selected());
    }

    #[test]
    fn test_handle_action_preview() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        assert!(app.handle_action(Action::Preview));
        assert_eq!(app.mode(), AppMode::Previewing);
    }

    #[test]
    fn test_handle_action_delete_requires_selection() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        // Without selection, delete should not work
        assert!(!app.handle_action(Action::Delete));
        assert_eq!(app.mode(), AppMode::Reviewing);

        // With selection, delete should transition to Confirming
        app.toggle_select();
        assert!(app.handle_action(Action::Delete));
        assert_eq!(app.mode(), AppMode::Confirming);
    }

    #[test]
    fn test_handle_action_cancel() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        app.set_mode(AppMode::Previewing);
        assert!(app.handle_action(Action::Cancel));
        assert_eq!(app.mode(), AppMode::Reviewing);

        app.toggle_select();
        app.set_mode(AppMode::Confirming);
        assert!(app.handle_action(Action::Cancel));
        assert_eq!(app.mode(), AppMode::Reviewing);
    }

    #[test]
    fn test_handle_action_quit() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        assert!(app.handle_action(Action::Quit));
        assert!(app.should_quit());
    }

    #[test]
    fn test_scan_progress() {
        let mut app = App::new();

        app.update_scan_progress("Walking", 50, 100, "/some/path/file.txt");

        let progress = app.scan_progress();
        assert_eq!(progress.phase, "Walking");
        assert_eq!(progress.current, 50);
        assert_eq!(progress.total, 100);
        assert_eq!(progress.percentage(), 50);
    }

    #[test]
    fn test_scan_progress_percentage() {
        let mut progress = ScanProgress::new();

        // 0 total should return 0%
        assert_eq!(progress.percentage(), 0);

        progress.total = 100;
        progress.current = 25;
        assert_eq!(progress.percentage(), 25);

        progress.current = 100;
        assert_eq!(progress.percentage(), 100);

        // Over 100% should cap at 100
        progress.current = 150;
        assert_eq!(progress.percentage(), 100);
    }

    #[test]
    fn test_error_handling() {
        let mut app = App::new();

        assert!(app.error_message().is_none());

        app.set_error("Something went wrong");
        assert_eq!(app.error_message(), Some("Something went wrong"));

        app.clear_error();
        assert!(app.error_message().is_none());
    }

    #[test]
    fn test_preview_handling() {
        let mut app = App::new();

        assert!(app.preview_content().is_none());

        app.set_preview("File content here".to_string());
        assert_eq!(app.preview_content(), Some("File content here"));

        app.clear_preview();
        assert!(app.preview_content().is_none());
    }

    #[test]
    fn test_app_mode_is_navigable() {
        assert!(!AppMode::Scanning.is_navigable());
        assert!(AppMode::Reviewing.is_navigable());
        assert!(!AppMode::Previewing.is_navigable());
        assert!(!AppMode::Confirming.is_navigable());
        assert!(!AppMode::Quitting.is_navigable());
    }

    #[test]
    fn test_app_mode_is_done() {
        assert!(!AppMode::Scanning.is_done());
        assert!(!AppMode::Reviewing.is_done());
        assert!(!AppMode::Previewing.is_done());
        assert!(!AppMode::Confirming.is_done());
        assert!(AppMode::Quitting.is_done());
    }

    #[test]
    fn test_duplicate_file_count() {
        let groups = vec![
            make_group(100, vec!["/a.txt", "/b.txt"]), // 2 files
            make_group(200, vec!["/c.txt", "/d.txt", "/e.txt"]), // 3 files
        ];
        let app = App::with_groups(groups);

        assert_eq!(app.duplicate_file_count(), 5);
    }

    #[test]
    fn test_scroll_update_on_navigation() {
        let paths: Vec<&str> = (0..30)
            .map(|i| Box::leak(format!("/file{}.txt", i).into_boxed_str()) as &str)
            .collect();
        let groups = vec![make_group(100, paths)];
        let mut app = App::with_groups(groups);
        app.set_visible_rows(10);

        // Navigate past visible area
        for _ in 0..15 {
            app.next();
        }

        // Scroll should have adjusted
        assert!(app.file_scroll() > 0);
        assert!(app.file_index() >= app.file_scroll());
        assert!(app.file_index() < app.file_scroll() + app.visible_rows);
    }
}
