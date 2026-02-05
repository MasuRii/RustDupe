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
//!         vec![
//!             rustdupe::scanner::FileEntry::new(PathBuf::from("/a.txt"), 1000, std::time::SystemTime::now()),
//!             rustdupe::scanner::FileEntry::new(PathBuf::from("/b.txt"), 1000, std::time::SystemTime::now()),
//!         ],
//!         vec![],
//!     ),
//! ];
//! app.set_groups(groups);
//! app.set_mode(AppMode::Reviewing);
//!
//! // Navigate and select
//! app.handle_action(rustdupe::tui::Action::ExpandAll);
//! app.next();
//! app.toggle_select();
//!
//! assert!(app.is_file_selected(&PathBuf::from("/b.txt")));
//! ```

use std::collections::HashSet;
use std::path::PathBuf;

use crate::cli::ThemeArg;
use crate::duplicates::DuplicateGroup;
use crate::tui::theme::Theme;

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
    /// Confirming a bulk selection operation
    ConfirmingBulkSelection,
    /// Selecting a folder for batch selection
    SelectingFolder,
    /// Selecting a named group for batch selection
    SelectingGroup,
    /// Inputting an extension for bulk selection
    InputtingExtension,
    /// Inputting a directory for bulk selection
    InputtingDirectory,
    /// Searching duplicate groups
    Searching,
    /// Showing help overlay with keybinding reference
    ShowingHelp,
    /// Application is quitting
    Quitting,
}

impl AppMode {
    /// Check if the application is in a navigable state.
    #[must_use]
    pub fn is_navigable(&self) -> bool {
        matches!(
            self,
            Self::Reviewing
                | Self::SelectingFolder
                | Self::SelectingGroup
                | Self::Searching
                | Self::InputtingExtension
                | Self::InputtingDirectory
        )
    }

    /// Check if the application is done (quitting).
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Quitting)
    }

    /// Check if the application is in a modal state.
    #[must_use]
    pub fn is_modal(&self) -> bool {
        matches!(
            self,
            Self::Previewing
                | Self::Confirming
                | Self::ConfirmingBulkSelection
                | Self::SelectingFolder
                | Self::SelectingGroup
                | Self::InputtingExtension
                | Self::InputtingDirectory
                | Self::ShowingHelp
        )
    }
}

/// User action triggered by keyboard input.
///
/// Actions are the result of key event processing and represent
/// user intentions that modify application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Navigate up in the list
    NavigateUp,
    /// Navigate down in the list
    NavigateDown,
    /// Navigate to next group
    NextGroup,
    /// Navigate to previous group
    PreviousGroup,
    /// Navigate to the first item (top of list)
    GoToTop,
    /// Navigate to the last item (bottom of list)
    GoToBottom,
    /// Toggle selection of current item
    ToggleSelect,
    /// Select all files in current group (except first)
    SelectAllInGroup,
    /// Select all duplicates across ALL groups (keeping first in each)
    SelectAllDuplicates,
    /// Select oldest file in each group (keep newest)
    SelectOldest,
    /// Select newest file in each group (keep oldest)
    SelectNewest,
    /// Select smallest file in each group (actually selects all but first since they match)
    SelectSmallest,
    /// Select largest file in each group (actually selects all but first since they match)
    SelectLargest,
    /// Select files by extension (global)
    SelectByExtension,
    /// Select files by directory (global)
    SelectByDirectory,
    /// Undo last bulk selection action
    UndoSelection,
    /// Deselect all files
    DeselectAll,
    /// Preview the selected file
    Preview,
    /// Enter folder selection mode
    SelectFolder,
    /// Enter named group selection mode
    SelectGroup,
    /// Enter search mode
    Search,
    /// Delete selected files (to trash)
    Delete,
    /// Toggle theme
    ToggleTheme,
    /// Toggle expand/collapse of current group
    ToggleExpand,
    /// Expand all groups
    ExpandAll,
    /// Collapse all groups
    CollapseAll,
    /// Toggle expand/collapse of all groups
    ToggleExpandAll,
    /// Cycle sort column
    CycleSortColumn,
    /// Reverse sort direction
    ReverseSortDirection,
    /// Show help overlay with keybinding reference
    ShowHelp,
    /// Confirm current action
    Confirm,
    /// Cancel current action
    Cancel,
    /// Quit the application
    Quit,
}

impl Action {
    /// Returns the snake_case name of the action (for config files).
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::Action;
    ///
    /// assert_eq!(Action::NavigateDown.name(), "navigate_down");
    /// assert_eq!(Action::GoToTop.name(), "go_to_top");
    /// ```
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::NavigateUp => "navigate_up",
            Self::NavigateDown => "navigate_down",
            Self::NextGroup => "next_group",
            Self::PreviousGroup => "previous_group",
            Self::GoToTop => "go_to_top",
            Self::GoToBottom => "go_to_bottom",
            Self::ToggleSelect => "toggle_select",
            Self::SelectAllInGroup => "select_all_in_group",
            Self::SelectAllDuplicates => "select_all_duplicates",
            Self::SelectOldest => "select_oldest",
            Self::SelectNewest => "select_newest",
            Self::SelectSmallest => "select_smallest",
            Self::SelectLargest => "select_largest",
            Self::SelectByExtension => "select_by_extension",
            Self::SelectByDirectory => "select_by_directory",
            Self::UndoSelection => "undo_selection",
            Self::DeselectAll => "deselect_all",
            Self::Preview => "preview",
            Self::SelectFolder => "select_folder",
            Self::SelectGroup => "select_group",
            Self::Search => "search",
            Self::Delete => "delete",
            Self::ToggleTheme => "toggle_theme",
            Self::ToggleExpand => "toggle_expand",
            Self::ExpandAll => "expand_all",
            Self::CollapseAll => "collapse_all",
            Self::ToggleExpandAll => "toggle_expand_all",
            Self::CycleSortColumn => "cycle_sort_column",
            Self::ReverseSortDirection => "reverse_sort_direction",
            Self::ShowHelp => "show_help",
            Self::Confirm => "confirm",
            Self::Cancel => "cancel",
            Self::Quit => "quit",
        }
    }

    /// Returns a list of all valid action names.
    #[must_use]
    pub fn all_names() -> Vec<&'static str> {
        vec![
            "navigate_up",
            "navigate_down",
            "next_group",
            "previous_group",
            "go_to_top",
            "go_to_bottom",
            "toggle_select",
            "select_all_in_group",
            "select_all_duplicates",
            "select_oldest",
            "select_newest",
            "select_smallest",
            "select_largest",
            "select_by_extension",
            "select_by_directory",
            "undo_selection",
            "deselect_all",
            "preview",
            "select_folder",
            "select_group",
            "search",
            "delete",
            "toggle_theme",
            "toggle_expand",
            "expand_all",
            "collapse_all",
            "toggle_expand_all",
            "cycle_sort_column",
            "reverse_sort_direction",
            "show_help",
            "confirm",
            "cancel",
            "quit",
        ]
    }

    /// Returns all action variants.
    #[must_use]
    pub const fn all() -> [Action; 33] {
        [
            Self::NavigateUp,
            Self::NavigateDown,
            Self::NextGroup,
            Self::PreviousGroup,
            Self::GoToTop,
            Self::GoToBottom,
            Self::ToggleSelect,
            Self::SelectAllInGroup,
            Self::SelectAllDuplicates,
            Self::SelectOldest,
            Self::SelectNewest,
            Self::SelectSmallest,
            Self::SelectLargest,
            Self::SelectByExtension,
            Self::SelectByDirectory,
            Self::UndoSelection,
            Self::DeselectAll,
            Self::Preview,
            Self::SelectFolder,
            Self::SelectGroup,
            Self::Search,
            Self::Delete,
            Self::ToggleTheme,
            Self::ToggleExpand,
            Self::ExpandAll,
            Self::CollapseAll,
            Self::ToggleExpandAll,
            Self::CycleSortColumn,
            Self::ReverseSortDirection,
            Self::ShowHelp,
            Self::Confirm,
            Self::Cancel,
            Self::Quit,
        ]
    }
}

impl std::str::FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "navigate_up" | "up" => Ok(Self::NavigateUp),
            "navigate_down" | "down" => Ok(Self::NavigateDown),
            "next_group" => Ok(Self::NextGroup),
            "previous_group" | "prev_group" => Ok(Self::PreviousGroup),
            "go_to_top" | "top" => Ok(Self::GoToTop),
            "go_to_bottom" | "bottom" => Ok(Self::GoToBottom),
            "toggle_select" | "select" => Ok(Self::ToggleSelect),
            "select_all_in_group" | "select_all_group" => Ok(Self::SelectAllInGroup),
            "select_all_duplicates" | "select_all" => Ok(Self::SelectAllDuplicates),
            "select_oldest" | "oldest" => Ok(Self::SelectOldest),
            "select_newest" | "newest" => Ok(Self::SelectNewest),
            "select_smallest" | "smallest" => Ok(Self::SelectSmallest),
            "select_largest" | "largest" => Ok(Self::SelectLargest),
            "select_by_extension" | "extension" => Ok(Self::SelectByExtension),
            "select_by_directory" | "directory" => Ok(Self::SelectByDirectory),
            "undo_selection" | "undo" => Ok(Self::UndoSelection),
            "deselect_all" | "deselect" => Ok(Self::DeselectAll),
            "preview" => Ok(Self::Preview),
            "select_folder" | "folder" => Ok(Self::SelectFolder),
            "select_group" | "group" => Ok(Self::SelectGroup),
            "search" | "/" => Ok(Self::Search),
            "delete" => Ok(Self::Delete),
            "toggle_theme" | "theme" => Ok(Self::ToggleTheme),
            "toggle_expand" | "expand" | "collapse" => Ok(Self::ToggleExpand),
            "expand_all" => Ok(Self::ExpandAll),
            "collapse_all" => Ok(Self::CollapseAll),
            "toggle_expand_all" | "toggle_all" => Ok(Self::ToggleExpandAll),
            "cycle_sort_column" | "sort" | "tab" => Ok(Self::CycleSortColumn),
            "reverse_sort_direction" | "reverse_sort" | "shift_tab" => {
                Ok(Self::ReverseSortDirection)
            }
            "show_help" | "help" => Ok(Self::ShowHelp),
            "confirm" | "enter" => Ok(Self::Confirm),
            "cancel" | "escape" | "esc" => Ok(Self::Cancel),
            "quit" | "exit" => Ok(Self::Quit),
            _ => Err(format!("Unknown action: '{s}'")),
        }
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
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
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::app::ScanProgress;
    /// let progress = ScanProgress::new();
    /// ```
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

/// Types of bulk selection actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkSelectionType {
    AllDuplicates,
    Oldest,
    Newest,
    Smallest,
    Largest,
    ByExtension,
    ByDirectory,
    InGroup,
    InFolder,
    InNamedGroup,
}

/// Column used for sorting duplicate groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortColumn {
    /// Sort by file size (largest groups first)
    #[default]
    Size,
    /// Sort by file path of the first file in group
    Path,
    /// Sort by modification date of the first file in group
    Date,
    /// Sort by number of duplicates in group
    Count,
}

impl SortColumn {
    /// Get the next column in rotation.
    #[must_use]
    pub fn next(&self) -> Self {
        match self {
            Self::Size => Self::Path,
            Self::Path => Self::Date,
            Self::Date => Self::Count,
            Self::Count => Self::Size,
        }
    }

    /// Get the display name of the column.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Size => "Size",
            Self::Path => "Path",
            Self::Date => "Date",
            Self::Count => "Count",
        }
    }
}

/// Direction for sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Sort in descending order (largest/newest/last first)
    #[default]
    Descending,
    /// Sort in ascending order (smallest/oldest/first first)
    Ascending,
}

impl SortDirection {
    /// Reverse the direction.
    #[must_use]
    pub fn reverse(&self) -> Self {
        match self {
            Self::Descending => Self::Ascending,
            Self::Ascending => Self::Descending,
        }
    }

    /// Get the display indicator.
    #[must_use]
    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Descending => "▼",
            Self::Ascending => "▲",
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
    /// Folder list for selection mode
    folder_list: Vec<PathBuf>,
    /// Currently selected folder index
    folder_index: usize,
    /// Named group list for selection mode (unique group names from all files)
    group_name_list: Vec<String>,
    /// Currently selected group name index
    group_name_index: usize,
    /// Search query string
    search_query: String,
    /// Input query for bulk selection
    input_query: String,
    /// Indices of groups matching the search query (None if no search active)
    filtered_indices: Option<Vec<usize>>,
    /// Protected reference paths
    reference_paths: Vec<PathBuf>,
    /// History of selections for undo
    selection_history: Vec<HashSet<PathBuf>>,
    /// Pending selections for preview
    pending_selections: HashSet<PathBuf>,
    /// Type of pending bulk selection
    pending_bulk_action: Option<BulkSelectionType>,
    /// Total reclaimable space in bytes
    reclaimable_space: u64,
    /// Number of visible rows in the UI (for scroll calculation)
    visible_rows: usize,
    /// Dry-run mode active (no deletions allowed)
    dry_run: bool,
    /// TUI theme setting
    theme_arg: ThemeArg,
    /// TUI theme colors
    theme: Theme,
    /// Active keybindings for display in help
    keybindings: Option<crate::tui::keybindings::KeyBindings>,
    /// Expanded group hashes
    expanded_groups: HashSet<[u8; 32]>,
    /// Column to sort groups by
    sort_column: SortColumn,
    /// Direction to sort groups
    sort_direction: SortDirection,
    /// Accessible mode for screen reader compatibility
    accessible: bool,
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
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::app::App;
    /// let app = App::new();
    /// ```
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
            folder_list: Vec::new(),
            folder_index: 0,
            group_name_list: Vec::new(),
            group_name_index: 0,
            search_query: String::new(),
            input_query: String::new(),
            filtered_indices: None,
            reference_paths: Vec::new(),
            selection_history: Vec::new(),
            pending_selections: HashSet::new(),
            pending_bulk_action: None,
            reclaimable_space: 0,
            visible_rows: 20, // Default, will be updated by UI
            dry_run: false,
            theme_arg: ThemeArg::Auto,
            theme: Theme::dark(),
            keybindings: None,
            expanded_groups: HashSet::new(),
            sort_column: SortColumn::default(),
            sort_direction: SortDirection::default(),
            accessible: false,
        }
    }

    /// Set theme for the application.
    pub fn with_theme(mut self, theme_arg: ThemeArg) -> Self {
        self.theme_arg = theme_arg;
        self.theme = match theme_arg {
            ThemeArg::Auto => Theme::auto(),
            ThemeArg::Light => Theme::light(),
            ThemeArg::Dark => Theme::dark(),
        };
        self
    }

    /// Toggle theme between light and dark.
    pub fn toggle_theme(&mut self) {
        self.theme_arg = match self.theme_arg {
            ThemeArg::Auto => {
                // If it was auto, switch to the opposite of what was detected
                if self.theme.is_light() {
                    ThemeArg::Dark
                } else {
                    ThemeArg::Light
                }
            }
            ThemeArg::Dark => ThemeArg::Light,
            ThemeArg::Light => ThemeArg::Dark,
        };

        self.theme = match self.theme_arg {
            ThemeArg::Light => Theme::light(),
            ThemeArg::Dark => Theme::dark(),
            ThemeArg::Auto => Theme::auto(), // Won't happen
        };

        log::debug!("Theme toggled to {:?}", self.theme_arg);
    }

    /// Get the current theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Set keybindings for help display.
    pub fn set_keybindings(&mut self, bindings: crate::tui::keybindings::KeyBindings) {
        self.keybindings = Some(bindings);
    }

    /// Set keybindings for help display (builder pattern).
    pub fn with_keybindings(mut self, bindings: crate::tui::keybindings::KeyBindings) -> Self {
        self.keybindings = Some(bindings);
        self
    }

    /// Get the current keybindings, if set.
    #[must_use]
    pub fn keybindings(&self) -> Option<&crate::tui::keybindings::KeyBindings> {
        self.keybindings.as_ref()
    }

    /// Set accessible mode for screen reader compatibility.
    ///
    /// When enabled:
    /// - Uses simple ASCII borders instead of Unicode box-drawing characters
    /// - Disables animations and spinners
    /// - Simplifies progress output
    pub fn with_accessible(mut self, accessible: bool) -> Self {
        self.accessible = accessible;
        self
    }

    /// Enable accessible mode.
    pub fn set_accessible(&mut self, accessible: bool) {
        self.accessible = accessible;
    }

    /// Check if accessible mode is enabled.
    #[must_use]
    pub fn is_accessible(&self) -> bool {
        self.accessible
    }

    /// Set dry-run mode for the application.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Check if dry-run mode is active.
    #[must_use]
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Set reference paths for the application.
    pub fn with_reference_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.reference_paths = paths;
        self
    }

    /// Set reference paths.
    pub fn set_reference_paths(&mut self, paths: Vec<PathBuf>) {
        self.reference_paths = paths;
    }

    /// Check if a path is in a protected reference directory.
    pub fn is_in_reference_dir(&self, path: &std::path::Path) -> bool {
        self.reference_paths.iter().any(|ref_path| {
            if cfg!(windows) {
                // Windows is case-insensitive. Convert to lowercase PathBuf for reliable
                // component-based comparison.
                let p = std::path::PathBuf::from(path.to_string_lossy().to_lowercase());
                let r = std::path::PathBuf::from(ref_path.to_string_lossy().to_lowercase());
                p.starts_with(r)
            } else {
                path.starts_with(ref_path)
            }
        })
    }

    /// Check if a group is expanded.
    #[must_use]
    pub fn is_expanded(&self, group_hash: &[u8; 32]) -> bool {
        self.expanded_groups.contains(group_hash)
    }

    /// Check if the currently selected group is expanded.
    #[must_use]
    pub fn is_current_group_expanded(&self) -> bool {
        self.current_group()
            .map(|g| self.is_expanded(&g.hash))
            .unwrap_or(false)
    }

    /// Create an App with pre-loaded duplicate groups.
    ///
    /// The app starts in Reviewing mode if groups are provided.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::app::App;
    /// use rustdupe::duplicates::DuplicateGroup;
    /// let app = App::with_groups(vec![]);
    /// ```
    #[must_use]
    pub fn with_groups(groups: Vec<DuplicateGroup>) -> Self {
        let reclaimable = groups.iter().map(DuplicateGroup::wasted_space).sum();
        let mode = if groups.is_empty() {
            AppMode::Scanning
        } else {
            AppMode::Reviewing
        };

        let mut app = Self {
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
            folder_list: Vec::new(),
            folder_index: 0,
            group_name_list: Vec::new(),
            group_name_index: 0,
            search_query: String::new(),
            input_query: String::new(),
            filtered_indices: None,
            reference_paths: Vec::new(),
            selection_history: Vec::new(),
            pending_selections: HashSet::new(),
            pending_bulk_action: None,
            reclaimable_space: reclaimable,
            visible_rows: 20,
            dry_run: false,
            theme_arg: ThemeArg::Auto,
            theme: Theme::dark(),
            keybindings: None,
            expanded_groups: HashSet::new(),
            sort_column: SortColumn::default(),
            sort_direction: SortDirection::default(),
            accessible: false,
        };

        if app.has_groups() {
            app.sort_groups();
            // Reset navigation to top after initial sort
            app.group_index = 0;
            app.file_index = 0;
            app.group_scroll = 0;
            app.file_scroll = 0;
        }

        app
    }

    /// Set selections and navigation state from a session.
    pub fn apply_session(
        &mut self,
        user_selections: std::collections::BTreeSet<PathBuf>,
        group_index: usize,
        file_index: usize,
    ) {
        self.selected_files = user_selections.into_iter().collect();

        // Validate group_index
        if group_index < self.groups.len() {
            self.group_index = group_index;

            // Validate file_index
            if file_index < self.groups[group_index].files.len() {
                self.file_index = file_index;
            } else {
                self.file_index = 0;
            }
        } else {
            self.group_index = 0;
            self.file_index = 0;
        }

        self.update_group_scroll();
        self.update_file_scroll();

        log::debug!(
            "Applied session: {} selections, pos ({}, {})",
            self.selected_files.len(),
            self.group_index,
            self.file_index
        );
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
        self.selected_files.clear();

        if !self.groups.is_empty() {
            self.sort_groups();
        }

        // Reset navigation to top after loading new groups
        self.group_index = 0;
        self.file_index = 0;
        self.group_scroll = 0;
        self.file_scroll = 0;

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

    /// Get the current navigation position as (group_index, file_index).
    #[must_use]
    pub fn navigation_position(&self) -> (usize, usize) {
        (self.group_index, self.file_index)
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
        self.visible_group_at(self.group_index)
    }

    /// Get the currently selected file path (if any).
    #[must_use]
    pub fn current_file(&self) -> Option<&PathBuf> {
        self.current_group()
            .and_then(|g| g.files.get(self.file_index))
            .map(|f| &f.path)
    }

    /// Get the currently selected file entry (if any).
    #[must_use]
    pub fn current_file_entry(&self) -> Option<&crate::scanner::FileEntry> {
        self.current_group()
            .and_then(|g| g.files.get(self.file_index))
    }

    /// Navigate to the next file in the current group.
    ///
    /// If at the end of the group, stays at the last file.
    pub fn next(&mut self) {
        if !self.mode.is_navigable() || self.visible_group_count() == 0 {
            return;
        }

        match self.mode {
            AppMode::Reviewing => {
                if let Some(group) = self.current_group() {
                    let is_expanded = self.is_expanded(&group.hash);
                    if is_expanded && self.file_index + 1 < group.files.len() {
                        self.file_index += 1;
                        self.update_file_scroll();
                        log::trace!("Navigate next: file_index = {}", self.file_index);
                    } else {
                        // If collapsed or at end of group, move to next group
                        self.next_group();
                    }
                }
            }
            AppMode::SelectingFolder => {
                if self.folder_index + 1 < self.folder_list.len() {
                    self.folder_index += 1;
                    log::trace!("Navigate next folder: folder_index = {}", self.folder_index);
                }
            }
            AppMode::SelectingGroup => {
                if self.group_name_index + 1 < self.group_name_list.len() {
                    self.group_name_index += 1;
                    log::trace!(
                        "Navigate next group name: group_name_index = {}",
                        self.group_name_index
                    );
                }
            }
            _ => {}
        }
    }

    /// Navigate to the previous file or folder.
    pub fn previous(&mut self) {
        if !self.mode.is_navigable() || self.visible_group_count() == 0 {
            return;
        }

        match self.mode {
            AppMode::Reviewing => {
                if let Some(group) = self.current_group() {
                    let is_expanded = self.is_expanded(&group.hash);
                    if is_expanded && self.file_index > 0 {
                        self.file_index -= 1;
                        self.update_file_scroll();
                        log::trace!("Navigate previous: file_index = {}", self.file_index);
                    } else {
                        // If collapsed or at start of group, move to previous group
                        let old_group_index = self.group_index;
                        self.previous_group();

                        // If we moved to a new group and it's expanded, go to its last file
                        if self.group_index != old_group_index {
                            if let Some(new_group) = self.current_group() {
                                if self.is_expanded(&new_group.hash) {
                                    self.file_index = new_group.files.len().saturating_sub(1);
                                    self.update_file_scroll();
                                }
                            }
                        }
                    }
                }
            }
            AppMode::SelectingFolder => {
                if self.folder_index > 0 {
                    self.folder_index -= 1;
                    log::trace!(
                        "Navigate previous folder: folder_index = {}",
                        self.folder_index
                    );
                }
            }
            AppMode::SelectingGroup => {
                if self.group_name_index > 0 {
                    self.group_name_index -= 1;
                    log::trace!(
                        "Navigate previous group name: group_name_index = {}",
                        self.group_name_index
                    );
                }
            }
            _ => {}
        }
    }

    /// Navigate to the next duplicate group.
    pub fn next_group(&mut self) {
        if !self.mode.is_navigable() || self.visible_group_count() == 0 {
            return;
        }

        if self.group_index + 1 < self.visible_group_count() {
            self.group_index += 1;
            self.file_index = 0;
            self.file_scroll = 0;
            self.update_group_scroll();
            log::trace!("Navigate next group: group_index = {}", self.group_index);
        }
    }

    /// Navigate to the previous duplicate group.
    pub fn previous_group(&mut self) {
        if !self.mode.is_navigable() || self.visible_group_count() == 0 {
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

    /// Get selected files as a BTreeSet for deterministic serialization.
    #[must_use]
    pub fn selected_files_btree(&self) -> std::collections::BTreeSet<PathBuf> {
        self.selected_files.iter().cloned().collect()
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
    /// Cannot select files in protected reference directories.
    pub fn toggle_select(&mut self) {
        if let Some(path) = self.current_file().cloned() {
            if self.is_in_reference_dir(&path) {
                self.set_error("Cannot select file in protected reference directory");
                return;
            }

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
    ///
    /// Note: This bypasses the reference directory check.
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
    /// Files in protected reference directories are skipped.
    pub fn select_all_in_group(&mut self) {
        self.push_selection_history();
        // Clone files to avoid borrow conflict
        let files_to_select: Vec<PathBuf> = self
            .current_group()
            .map(|g| {
                g.files
                    .iter()
                    .skip(1)
                    .filter(|f| !self.is_in_reference_dir(&f.path))
                    .map(|f| f.path.clone())
                    .collect()
            })
            .unwrap_or_default();

        let count = files_to_select.len();
        for path in files_to_select {
            self.selected_files.insert(path);
        }

        if count > 0 {
            log::debug!(
                "Selected {} files in group (keeping first and skipping references)",
                count
            );
        }
    }

    /// Select all duplicates across ALL groups (keeping first in each).
    pub fn select_all_duplicates(&mut self) {
        let mut pending = HashSet::new();
        for group in &self.groups {
            for file in group.files.iter().skip(1) {
                if !self.is_in_reference_dir(&file.path)
                    && !self.selected_files.contains(&file.path)
                {
                    pending.insert(file.path.clone());
                }
            }
        }

        if pending.is_empty() {
            log::debug!("No new duplicates to select");
            return;
        }

        self.pending_selections = pending;
        self.pending_bulk_action = Some(BulkSelectionType::AllDuplicates);
        self.set_mode(AppMode::ConfirmingBulkSelection);
    }

    /// Select the oldest file in each group (keeping the newest).
    pub fn select_oldest(&mut self) {
        let mut pending = HashSet::new();
        for group in &self.groups {
            // Find the newest file to keep
            if let Some(newest) = group.files.iter().max_by_key(|f| f.modified) {
                for file in &group.files {
                    if file.path != newest.path
                        && !self.is_in_reference_dir(&file.path)
                        && !self.selected_files.contains(&file.path)
                    {
                        pending.insert(file.path.clone());
                    }
                }
            }
        }

        if pending.is_empty() {
            log::debug!("No new oldest files to select");
            return;
        }

        self.pending_selections = pending;
        self.pending_bulk_action = Some(BulkSelectionType::Oldest);
        self.set_mode(AppMode::ConfirmingBulkSelection);
    }

    /// Select the newest file in each group (keeping the oldest).
    pub fn select_newest(&mut self) {
        let mut pending = HashSet::new();
        for group in &self.groups {
            // Find the oldest file to keep
            if let Some(oldest) = group.files.iter().min_by_key(|f| f.modified) {
                for file in &group.files {
                    if file.path != oldest.path
                        && !self.is_in_reference_dir(&file.path)
                        && !self.selected_files.contains(&file.path)
                    {
                        pending.insert(file.path.clone());
                    }
                }
            }
        }

        if pending.is_empty() {
            log::debug!("No new newest files to select");
            return;
        }

        self.pending_selections = pending;
        self.pending_bulk_action = Some(BulkSelectionType::Newest);
        self.set_mode(AppMode::ConfirmingBulkSelection);
    }

    /// Select all but the first file in each group (same size, so "smallest" is arbitrary).
    pub fn select_smallest(&mut self) {
        let mut pending = HashSet::new();
        for group in &self.groups {
            for file in group.files.iter().skip(1) {
                if !self.is_in_reference_dir(&file.path)
                    && !self.selected_files.contains(&file.path)
                {
                    pending.insert(file.path.clone());
                }
            }
        }

        if pending.is_empty() {
            log::debug!("No new duplicates to select (smallest)");
            return;
        }

        self.pending_selections = pending;
        self.pending_bulk_action = Some(BulkSelectionType::Smallest);
        self.set_mode(AppMode::ConfirmingBulkSelection);
    }

    /// Select all but the first file in each group (same size, so "largest" is arbitrary).
    pub fn select_largest(&mut self) {
        let mut pending = HashSet::new();
        for group in &self.groups {
            for file in group.files.iter().skip(1) {
                if !self.is_in_reference_dir(&file.path)
                    && !self.selected_files.contains(&file.path)
                {
                    pending.insert(file.path.clone());
                }
            }
        }

        if pending.is_empty() {
            log::debug!("No new duplicates to select (largest)");
            return;
        }

        self.pending_selections = pending;
        self.pending_bulk_action = Some(BulkSelectionType::Largest);
        self.set_mode(AppMode::ConfirmingBulkSelection);
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
        let deleted_set: std::collections::HashSet<&PathBuf> = deleted.iter().collect();

        // Remove from selection
        self.selected_files.retain(|p| !deleted_set.contains(p));

        // Remove from groups and filter empty groups
        for group in &mut self.groups {
            group.files.retain(|f| !deleted_set.contains(&f.path));
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

    // ==================== Bulk Selection ====================

    /// Save the current selection state to history for undo.
    fn push_selection_history(&mut self) {
        self.selection_history.push(self.selected_files.clone());
        // Limit history size
        if self.selection_history.len() > 50 {
            self.selection_history.remove(0);
        }
    }

    /// Undo the last bulk selection action.
    pub fn undo_selection(&mut self) {
        if let Some(previous) = self.selection_history.pop() {
            self.selected_files = previous;
            log::info!(
                "Undid last selection action. {} files selected",
                self.selected_files.len()
            );
        } else {
            self.set_error("Nothing to undo");
        }
    }

    /// Get the input query (for extension/directory input).
    #[must_use]
    pub fn input_query(&self) -> &str {
        &self.input_query
    }

    /// Set the input query.
    pub fn set_input_query(&mut self, query: String) {
        self.input_query = query;
    }

    /// Clear the input query.
    pub fn clear_input_query(&mut self) {
        self.input_query.clear();
    }

    // ==================== Sorting ====================

    /// Sort the duplicate groups based on current sort settings.
    pub fn sort_groups(&mut self) {
        if self.groups.is_empty() {
            return;
        }

        // Store current selection if possible to restore position
        let current_hash = self.current_group().map(|g| g.hash);

        match self.sort_column {
            SortColumn::Size => match self.sort_direction {
                SortDirection::Descending => self.groups.sort_by(|a, b| b.size.cmp(&a.size)),
                SortDirection::Ascending => self.groups.sort_by(|a, b| a.size.cmp(&b.size)),
            },
            SortColumn::Path => {
                self.groups.sort_by(|a, b| {
                    let path_a = a
                        .files
                        .first()
                        .map(|f| f.path.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let path_b = b
                        .files
                        .first()
                        .map(|f| f.path.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    match self.sort_direction {
                        SortDirection::Descending => path_b.cmp(&path_a),
                        SortDirection::Ascending => path_a.cmp(&path_b),
                    }
                });
            }
            SortColumn::Date => {
                self.groups.sort_by(|a, b| {
                    let date_a = a
                        .files
                        .first()
                        .map(|f| f.modified)
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let date_b = b
                        .files
                        .first()
                        .map(|f| f.modified)
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    match self.sort_direction {
                        SortDirection::Descending => date_b.cmp(&date_a),
                        SortDirection::Ascending => date_a.cmp(&date_b),
                    }
                });
            }
            SortColumn::Count => match self.sort_direction {
                SortDirection::Descending => self
                    .groups
                    .sort_by(|a, b| b.files.len().cmp(&a.files.len())),
                SortDirection::Ascending => self
                    .groups
                    .sort_by(|a, b| a.files.len().cmp(&b.files.len())),
            },
        }

        // If search is active, we MUST re-apply it because the original indices have changed
        if self.search_query.is_empty() {
            self.filtered_indices = None;
        } else {
            // Re-apply search logic without resetting navigation (yet)
            let query = self.search_query.to_lowercase();
            let re = regex::RegexBuilder::new(&self.search_query)
                .case_insensitive(true)
                .build()
                .ok();

            let indices: Vec<usize> = self
                .groups
                .iter()
                .enumerate()
                .filter(|(_, group)| {
                    group.files.iter().any(|file| {
                        let path_str = file.path.to_string_lossy();
                        let group_name = file.group_name.as_deref().unwrap_or("");
                        if let Some(ref r) = re {
                            if r.is_match(&path_str) || r.is_match(group_name) {
                                return true;
                            }
                        }
                        path_str.to_lowercase().contains(&query)
                            || group_name.to_lowercase().contains(&query)
                    })
                })
                .map(|(i, _)| i)
                .collect();
            self.filtered_indices = Some(indices);
        }

        // Restore position or reset
        if let Some(hash) = current_hash {
            if let Some(new_idx) = self.groups.iter().position(|g| g.hash == hash) {
                if let Some(ref filtered) = self.filtered_indices {
                    if let Some(filtered_idx) =
                        filtered.iter().position(|&orig_idx| orig_idx == new_idx)
                    {
                        self.group_index = filtered_idx;
                    } else {
                        self.group_index = 0;
                    }
                } else {
                    self.group_index = new_idx;
                }
            } else {
                self.group_index = 0;
            }
        } else {
            self.group_index = 0;
        }

        self.update_group_scroll();
        log::debug!(
            "Groups sorted by {:?} {:?}",
            self.sort_column,
            self.sort_direction
        );
    }

    /// Cycle to the next sort column.
    pub fn cycle_sort_column(&mut self) {
        self.sort_column = self.sort_column.next();
        self.sort_groups();
    }

    /// Reverse the current sort direction.
    pub fn reverse_sort_direction(&mut self) {
        self.sort_direction = self.sort_direction.reverse();
        self.sort_groups();
    }

    /// Get the current sort column.
    #[must_use]
    pub fn sort_column(&self) -> SortColumn {
        self.sort_column
    }

    /// Get the current sort direction.
    #[must_use]
    pub fn sort_direction(&self) -> SortDirection {
        self.sort_direction
    }

    /// Get the pending bulk selection count.
    #[must_use]
    pub fn pending_selection_count(&self) -> usize {
        self.pending_selections.len()
    }

    /// Get the type of pending bulk selection.
    #[must_use]
    pub fn pending_bulk_action(&self) -> Option<BulkSelectionType> {
        self.pending_bulk_action
    }

    /// Prepare a bulk selection by extension.
    pub fn prepare_select_by_extension(&mut self) {
        let ext = self.input_query.trim();
        if ext.is_empty() {
            self.set_mode(AppMode::Reviewing);
            return;
        }

        // Normalize extension: ensure it starts with dot if not empty
        let normalized_ext = if ext.starts_with('.') {
            ext.to_lowercase()
        } else {
            format!(".{}", ext.to_lowercase())
        };

        let mut pending = HashSet::new();
        for group in &self.groups {
            // Find files with extension
            let matching: Vec<_> = group
                .files
                .iter()
                .filter(|f| {
                    f.path.extension().is_some_and(|e| {
                        format!(".{}", e.to_string_lossy().to_lowercase()) == normalized_ext
                    })
                })
                .collect();

            if matching.is_empty() {
                continue;
            }

            // If ALL files in group match extension, we must keep at least one
            let skip_one = matching.len() >= group.files.len();

            for (i, file) in matching.into_iter().enumerate() {
                if skip_one && i == 0 {
                    continue;
                }
                if !self.is_in_reference_dir(&file.path) {
                    pending.insert(file.path.clone());
                }
            }
        }

        if pending.is_empty() {
            self.set_error(&format!(
                "No duplicates found with extension '{normalized_ext}'"
            ));
            self.set_mode(AppMode::Reviewing);
        } else {
            self.pending_selections = pending;
            self.pending_bulk_action = Some(BulkSelectionType::ByExtension);
            self.set_mode(AppMode::ConfirmingBulkSelection);
        }
    }

    /// Prepare a bulk selection by directory.
    pub fn prepare_select_by_directory(&mut self) {
        let dir_str = self.input_query.trim();
        if dir_str.is_empty() {
            self.set_mode(AppMode::Reviewing);
            return;
        }

        let dir_path = PathBuf::from(dir_str);

        let mut pending = HashSet::new();
        for group in &self.groups {
            let matching: Vec<_> = group
                .files
                .iter()
                .filter(|f| f.path.starts_with(&dir_path))
                .collect();

            if matching.is_empty() {
                continue;
            }

            // If ALL files in group match directory, we must keep at least one
            let skip_one = matching.len() >= group.files.len();

            for (i, file) in matching.into_iter().enumerate() {
                if skip_one && i == 0 {
                    continue;
                }
                if !self.is_in_reference_dir(&file.path) {
                    pending.insert(file.path.clone());
                }
            }
        }

        if pending.is_empty() {
            self.set_error(&format!(
                "No duplicates found in directory '{}'",
                dir_path.display()
            ));
            self.set_mode(AppMode::Reviewing);
        } else {
            self.pending_selections = pending;
            self.pending_bulk_action = Some(BulkSelectionType::ByDirectory);
            self.set_mode(AppMode::ConfirmingBulkSelection);
        }
    }

    /// Apply the pending bulk selection.
    pub fn apply_bulk_selection(&mut self) {
        if self.pending_selections.is_empty() {
            self.set_mode(AppMode::Reviewing);
            return;
        }

        self.push_selection_history();
        let count = self.pending_selections.len();
        for path in self.pending_selections.drain() {
            self.selected_files.insert(path);
        }

        log::info!("Applied bulk selection: {} files selected", count);
        self.pending_bulk_action = None;
        self.set_mode(AppMode::Reviewing);
    }

    /// Cancel the pending bulk selection.
    pub fn cancel_bulk_selection(&mut self) {
        self.pending_selections.clear();
        self.pending_bulk_action = None;
        self.set_mode(AppMode::Reviewing);
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

    // ==================== Search Management ====================

    /// Get the search query.
    #[must_use]
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Set the search query and update filters.
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.apply_search();
    }

    /// Check if a search is active.
    #[must_use]
    pub fn is_searching(&self) -> bool {
        self.filtered_indices.is_some()
    }

    /// Clear the search query and filter.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.filtered_indices = None;
        self.group_index = 0;
        self.file_index = 0;
        self.group_scroll = 0;
        self.file_scroll = 0;
    }

    /// Apply the current search query to the groups.
    fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = None;
        } else {
            let query = self.search_query.to_lowercase();

            // Try to compile as regex if it looks like one, or just use substring
            // We treat it as regex if it contains special chars or if it compiles successfully
            let re = regex::RegexBuilder::new(&self.search_query)
                .case_insensitive(true)
                .build()
                .ok();

            let indices: Vec<usize> = self
                .groups
                .iter()
                .enumerate()
                .filter(|(_, group)| {
                    // Match by filename, path, or group name
                    group.files.iter().any(|file| {
                        let path_str = file.path.to_string_lossy();
                        let group_name = file.group_name.as_deref().unwrap_or("");

                        if let Some(ref r) = re {
                            if r.is_match(&path_str) || r.is_match(group_name) {
                                return true;
                            }
                        }

                        // Fallback to substring matching if regex doesn't match or wasn't used
                        path_str.to_lowercase().contains(&query)
                            || group_name.to_lowercase().contains(&query)
                    })
                })
                .map(|(i, _)| i)
                .collect();
            self.filtered_indices = Some(indices);
        }

        // Reset navigation to the first match
        self.group_index = 0;
        self.file_index = 0;
        self.group_scroll = 0;
        self.file_scroll = 0;
    }

    /// Get the number of visible groups (filtered if search active).
    #[must_use]
    pub fn visible_group_count(&self) -> usize {
        self.filtered_indices
            .as_ref()
            .map_or(self.groups.len(), |v| v.len())
    }

    /// Get a visible group by its relative index.
    #[must_use]
    pub fn visible_group_at(&self, index: usize) -> Option<&DuplicateGroup> {
        let actual_idx = match &self.filtered_indices {
            Some(indices) => *indices.get(index)?,
            None => index,
        };
        self.groups.get(actual_idx)
    }

    // ==================== Folder Selection ====================

    /// Get the list of folders in the current group.
    #[must_use]
    pub fn folder_list(&self) -> &[PathBuf] {
        &self.folder_list
    }

    /// Get the currently selected folder index.
    #[must_use]
    pub fn folder_index(&self) -> usize {
        self.folder_index
    }

    /// Enter folder selection mode for the current group.
    pub fn enter_folder_selection(&mut self) {
        if let Some(group) = self.current_group() {
            let mut folders: Vec<PathBuf> = group
                .files
                .iter()
                .filter_map(|f| f.path.parent().map(|p| p.to_path_buf()))
                .collect();
            folders.sort();
            folders.dedup();
            self.folder_list = folders;
            self.folder_index = 0;
            self.set_mode(AppMode::SelectingFolder);
        }
    }

    /// Select all files in the current group that are within the selected folder.
    pub fn select_by_folder(&mut self) {
        let folder = match self.folder_list.get(self.folder_index) {
            Some(f) => f.clone(),
            None => {
                self.set_mode(AppMode::Reviewing);
                return;
            }
        };

        let files_to_select: Vec<PathBuf> = if let Some(group) = self.current_group() {
            // Ensure we don't select ALL files in the group
            let in_folder_count = group
                .files
                .iter()
                .filter(|f| f.path.starts_with(&folder))
                .count();

            if in_folder_count >= group.files.len() {
                self.set_error("Cannot select all files in group - at least one must be preserved");
                self.set_mode(AppMode::Reviewing);
                return;
            }

            group
                .files
                .iter()
                .filter(|f| f.path.starts_with(&folder) && !self.is_in_reference_dir(&f.path))
                .map(|f| f.path.clone())
                .collect()
        } else {
            Vec::new()
        };

        let count = files_to_select.len();
        for path in files_to_select {
            self.selected_files.insert(path);
        }

        if count > 0 {
            log::info!(
                "Selected {} files in folder {} (skipping references)",
                count,
                folder.display()
            );
        }
        self.set_mode(AppMode::Reviewing);
    }

    // ==================== Named Group Selection ====================

    /// Get the list of unique group names across all files.
    #[must_use]
    pub fn group_name_list(&self) -> &[String] {
        &self.group_name_list
    }

    /// Get the currently selected group name index.
    #[must_use]
    pub fn group_name_index(&self) -> usize {
        self.group_name_index
    }

    /// Enter group name selection mode.
    ///
    /// Collects all unique group names from files across all duplicate groups.
    pub fn enter_group_selection(&mut self) {
        let mut names: Vec<String> = self
            .groups
            .iter()
            .flat_map(|g| g.files.iter())
            .filter_map(|f| f.group_name.clone())
            .collect();
        names.sort();
        names.dedup();

        if names.is_empty() {
            self.set_error("No named groups found - use --group NAME=PATH when scanning");
            return;
        }

        self.group_name_list = names;
        self.group_name_index = 0;
        self.set_mode(AppMode::SelectingGroup);
    }

    /// Select all files in the current duplicate group that belong to the selected named group.
    ///
    /// This selects files based on their `group_name` field within the currently viewed
    /// duplicate group only, to avoid selecting all copies of a file across different groups.
    pub fn select_by_group_name(&mut self) {
        let group_name = match self.group_name_list.get(self.group_name_index) {
            Some(n) => n.clone(),
            None => {
                self.set_mode(AppMode::Reviewing);
                return;
            }
        };

        let files_to_select: Vec<PathBuf> = if let Some(group) = self.current_group() {
            // Ensure we don't select ALL files in the group
            let in_group_count = group
                .files
                .iter()
                .filter(|f| f.group_name.as_ref() == Some(&group_name))
                .count();

            if in_group_count >= group.files.len() {
                self.set_error("Cannot select all files in group - at least one must be preserved");
                self.set_mode(AppMode::Reviewing);
                return;
            }

            group
                .files
                .iter()
                .filter(|f| {
                    f.group_name.as_ref() == Some(&group_name) && !self.is_in_reference_dir(&f.path)
                })
                .map(|f| f.path.clone())
                .collect()
        } else {
            Vec::new()
        };

        let count = files_to_select.len();
        for path in files_to_select {
            self.selected_files.insert(path);
        }

        if count > 0 {
            log::info!(
                "Selected {} files in named group '{}' (skipping references)",
                count,
                group_name
            );
        }
        self.set_mode(AppMode::Reviewing);
    }

    /// Select all files across ALL duplicate groups that belong to the given named group.
    ///
    /// This is useful for batch operations like "delete all duplicates from the 'downloads' group".
    /// Files in reference directories are skipped.
    pub fn select_all_by_group_name(&mut self, group_name: &str) {
        self.push_selection_history();
        let mut count = 0;
        for group in &self.groups {
            // Ensure we don't select ALL files in any group
            let in_group_count = group
                .files
                .iter()
                .filter(|f| f.group_name.as_ref().is_some_and(|n| n == group_name))
                .count();

            // If all files in this duplicate group are from the named group, skip one
            let skip_first = in_group_count >= group.files.len();

            for (i, file) in group.files.iter().enumerate() {
                if file.group_name.as_ref().is_some_and(|n| n == group_name) {
                    if skip_first && i == 0 {
                        continue; // Skip first to preserve at least one
                    }
                    if !self.is_in_reference_dir(&file.path)
                        && self.selected_files.insert(file.path.clone())
                    {
                        count += 1;
                    }
                }
            }
        }

        log::info!(
            "Selected {} files across all groups in named group '{}'",
            count,
            group_name
        );
    }

    // ==================== Action Handling ====================

    /// Navigate to the first item (top of list).
    pub fn go_to_top(&mut self) {
        if !self.mode.is_navigable() || self.visible_group_count() == 0 {
            return;
        }

        match self.mode {
            AppMode::Reviewing => {
                self.file_index = 0;
                self.file_scroll = 0;
                log::trace!("Navigate to top: file_index = 0");
            }
            AppMode::SelectingFolder => {
                self.folder_index = 0;
                log::trace!("Navigate to top folder: folder_index = 0");
            }
            AppMode::SelectingGroup => {
                self.group_name_index = 0;
                log::trace!("Navigate to top group name: group_name_index = 0");
            }
            _ => {}
        }
    }

    /// Navigate to the last item (bottom of list).
    pub fn go_to_bottom(&mut self) {
        if !self.mode.is_navigable() || self.visible_group_count() == 0 {
            return;
        }

        match self.mode {
            AppMode::Reviewing => {
                if let Some(group) = self.current_group() {
                    let last_index = group.files.len().saturating_sub(1);
                    self.file_index = last_index;
                    self.update_file_scroll();
                    log::trace!("Navigate to bottom: file_index = {}", self.file_index);
                }
            }
            AppMode::SelectingFolder => {
                let last_index = self.folder_list.len().saturating_sub(1);
                self.folder_index = last_index;
                log::trace!(
                    "Navigate to bottom folder: folder_index = {}",
                    self.folder_index
                );
            }
            AppMode::SelectingGroup => {
                let last_index = self.group_name_list.len().saturating_sub(1);
                self.group_name_index = last_index;
                log::trace!(
                    "Navigate to bottom group name: group_name_index = {}",
                    self.group_name_index
                );
            }
            _ => {}
        }
    }

    /// Handle a user action and update state accordingly.
    ///
    /// Returns true if the action was handled.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::app::{App, Action};
    /// let mut app = App::new();
    /// app.handle_action(Action::Quit);
    /// assert!(app.should_quit());
    /// ```
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
            Action::GoToTop => {
                self.go_to_top();
                true
            }
            Action::GoToBottom => {
                self.go_to_bottom();
                true
            }
            Action::ToggleSelect => {
                if let Some(group) = self.current_group() {
                    let hash = group.hash;
                    if !self.expanded_groups.contains(&hash) {
                        self.expanded_groups.insert(hash);
                    } else if self.file_index == 0 {
                        // On first file, space collapses
                        self.expanded_groups.remove(&hash);
                    } else {
                        self.toggle_select();
                    }
                }
                true
            }
            Action::SelectAllInGroup => {
                self.select_all_in_group();
                true
            }
            Action::SelectAllDuplicates => {
                self.select_all_duplicates();
                true
            }
            Action::SelectOldest => {
                self.select_oldest();
                true
            }
            Action::SelectNewest => {
                self.select_newest();
                true
            }
            Action::SelectSmallest => {
                self.select_smallest();
                true
            }
            Action::SelectLargest => {
                self.select_largest();
                true
            }
            Action::SelectByExtension => {
                if self.mode == AppMode::Reviewing {
                    self.input_query.clear();
                    self.set_mode(AppMode::InputtingExtension);
                    true
                } else {
                    false
                }
            }
            Action::SelectByDirectory => {
                if self.mode == AppMode::Reviewing {
                    self.input_query.clear();
                    self.set_mode(AppMode::InputtingDirectory);
                    true
                } else {
                    false
                }
            }
            Action::UndoSelection => {
                self.undo_selection();
                true
            }
            Action::DeselectAll => {
                self.push_selection_history();
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
            Action::SelectFolder => {
                if self.mode == AppMode::Reviewing && self.current_group().is_some() {
                    self.enter_folder_selection();
                    true
                } else {
                    false
                }
            }
            Action::SelectGroup => {
                if self.mode == AppMode::Reviewing && self.current_group().is_some() {
                    self.enter_group_selection();
                    true
                } else {
                    false
                }
            }
            Action::Search => {
                if self.mode == AppMode::Reviewing {
                    self.set_mode(AppMode::Searching);
                    true
                } else {
                    false
                }
            }
            Action::Delete => {
                if self.dry_run {
                    self.set_error("Cannot delete files in dry-run mode");
                    return true; // Action handled (but blocked)
                }
                if self.mode == AppMode::Reviewing && self.has_selections() {
                    self.set_mode(AppMode::Confirming);
                    true
                } else {
                    false
                }
            }
            Action::ToggleTheme => {
                self.toggle_theme();
                true
            }
            Action::ToggleExpand => {
                if let Some(group) = self.current_group() {
                    let hash = group.hash;
                    if self.expanded_groups.contains(&hash) {
                        self.expanded_groups.remove(&hash);
                    } else {
                        self.expanded_groups.insert(hash);
                    }
                }
                true
            }
            Action::ExpandAll => {
                for group in &self.groups {
                    self.expanded_groups.insert(group.hash);
                }
                true
            }
            Action::CollapseAll => {
                self.expanded_groups.clear();
                true
            }
            Action::ToggleExpandAll => {
                if self.expanded_groups.len() >= self.groups.len() {
                    self.expanded_groups.clear();
                } else {
                    for group in &self.groups {
                        self.expanded_groups.insert(group.hash);
                    }
                }
                true
            }
            Action::CycleSortColumn => {
                self.cycle_sort_column();
                true
            }
            Action::ReverseSortDirection => {
                self.reverse_sort_direction();
                true
            }
            Action::ShowHelp => {
                if self.mode == AppMode::ShowingHelp {
                    // Toggle off - return to reviewing
                    self.set_mode(AppMode::Reviewing);
                } else if self.mode == AppMode::Reviewing {
                    self.set_mode(AppMode::ShowingHelp);
                }
                true
            }
            Action::Confirm => {
                if self.mode == AppMode::SelectingFolder {
                    self.push_selection_history();
                    self.select_by_folder();
                    true
                } else if self.mode == AppMode::SelectingGroup {
                    self.push_selection_history();
                    self.select_by_group_name();
                    true
                } else if self.mode == AppMode::InputtingExtension {
                    self.prepare_select_by_extension();
                    true
                } else if self.mode == AppMode::InputtingDirectory {
                    self.prepare_select_by_directory();
                    true
                } else if self.mode == AppMode::ConfirmingBulkSelection {
                    self.apply_bulk_selection();
                    true
                } else if self.mode == AppMode::Searching {
                    self.set_mode(AppMode::Reviewing);
                    true
                } else if self.mode == AppMode::Reviewing {
                    if let Some(group) = self.current_group() {
                        let hash = group.hash;
                        if self.expanded_groups.contains(&hash) {
                            self.expanded_groups.remove(&hash);
                        } else {
                            self.expanded_groups.insert(hash);
                        }
                    }
                    true
                } else {
                    // Confirmation handling is done by the TUI main loop
                    true
                }
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
                    AppMode::ConfirmingBulkSelection => {
                        self.cancel_bulk_selection();
                    }
                    AppMode::SelectingFolder => {
                        self.set_mode(AppMode::Reviewing);
                    }
                    AppMode::SelectingGroup => {
                        self.set_mode(AppMode::Reviewing);
                    }
                    AppMode::InputtingExtension | AppMode::InputtingDirectory => {
                        self.clear_input_query();
                        self.set_mode(AppMode::Reviewing);
                    }
                    AppMode::Searching => {
                        self.clear_search();
                        self.set_mode(AppMode::Reviewing);
                    }
                    AppMode::ShowingHelp => {
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
        let mut hash = [0u8; 32];
        let size_bytes = size.to_be_bytes();
        hash[..8].copy_from_slice(&size_bytes);

        DuplicateGroup::new(
            hash,
            size,
            paths
                .into_iter()
                .map(|p| {
                    crate::scanner::FileEntry::new(
                        PathBuf::from(p),
                        size,
                        std::time::SystemTime::now(),
                    )
                })
                .collect(),
            Vec::new(),
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

        // Initially collapsed, next() should move to next group (but there is none)
        app.next();
        assert_eq!(app.file_index(), 0);

        // Expand group to navigate files
        app.handle_action(Action::ToggleExpandAll);

        app.next();
        assert_eq!(app.file_index(), 1);

        app.next();
        assert_eq!(app.file_index(), 2);

        // At end, should stay at last (or move to next group if exists)
        app.next();
        assert_eq!(app.file_index(), 2);

        app.previous();
        assert_eq!(app.file_index(), 1);

        app.previous();
        assert_eq!(app.file_index(), 0);

        // At start, should stay at first (or move to prev group)
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
    fn test_folder_selection() {
        let groups = vec![make_group(
            100,
            vec!["/dir1/a.txt", "/dir1/b.txt", "/dir2/c.txt"],
        )];
        let mut app = App::with_groups(groups);

        app.enter_folder_selection();
        assert_eq!(app.mode(), AppMode::SelectingFolder);
        assert_eq!(app.folder_list().len(), 2);
        assert!(app.folder_list().contains(&PathBuf::from("/dir1")));
        assert!(app.folder_list().contains(&PathBuf::from("/dir2")));

        // Navigate to /dir1
        if app.folder_list()[0] != PathBuf::from("/dir1") {
            app.next();
        }
        assert_eq!(
            app.folder_list()[app.folder_index()],
            PathBuf::from("/dir1")
        );

        app.select_by_folder();
        assert_eq!(app.mode(), AppMode::Reviewing);
        assert!(app.is_file_selected(&PathBuf::from("/dir1/a.txt")));
        assert!(app.is_file_selected(&PathBuf::from("/dir1/b.txt")));
        assert!(!app.is_file_selected(&PathBuf::from("/dir2/c.txt")));
    }

    #[test]
    fn test_folder_selection_prevents_selecting_all() {
        let groups = vec![make_group(100, vec!["/dir1/a.txt", "/dir1/b.txt"])];
        let mut app = App::with_groups(groups);

        app.enter_folder_selection();
        app.select_by_folder();

        // Should have set an error and NOT selected anything (or at least not all)
        assert!(app.error_message().is_some());
        assert_eq!(app.selected_count(), 0);
    }

    fn make_group_with_names(
        size: u64,
        paths_and_names: Vec<(&str, Option<&str>)>,
    ) -> DuplicateGroup {
        DuplicateGroup::new(
            [0u8; 32],
            size,
            paths_and_names
                .into_iter()
                .map(|(path, group_name)| {
                    let mut entry = crate::scanner::FileEntry::new(
                        PathBuf::from(path),
                        size,
                        std::time::SystemTime::now(),
                    );
                    if let Some(name) = group_name {
                        entry.group_name = Some(name.to_string());
                    }
                    entry
                })
                .collect(),
            Vec::new(),
        )
    }

    #[test]
    fn test_group_name_selection() {
        let groups = vec![make_group_with_names(
            100,
            vec![
                ("/photos/a.jpg", Some("photos")),
                ("/docs/a.jpg", Some("docs")),
                ("/backup/a.jpg", Some("backup")),
            ],
        )];
        let mut app = App::with_groups(groups);

        app.enter_group_selection();
        assert_eq!(app.mode(), AppMode::SelectingGroup);
        assert_eq!(app.group_name_list().len(), 3);
        assert!(app.group_name_list().contains(&"photos".to_string()));
        assert!(app.group_name_list().contains(&"docs".to_string()));
        assert!(app.group_name_list().contains(&"backup".to_string()));

        // Navigate to 'docs' (groups are sorted, so: backup, docs, photos)
        assert_eq!(app.group_name_list()[1], "docs");
        app.next(); // Move from index 0 to 1
        assert_eq!(app.group_name_index(), 1);

        app.select_by_group_name();
        assert_eq!(app.mode(), AppMode::Reviewing);
        // Only the file with group_name "docs" should be selected
        assert!(!app.is_file_selected(&PathBuf::from("/photos/a.jpg")));
        assert!(app.is_file_selected(&PathBuf::from("/docs/a.jpg")));
        assert!(!app.is_file_selected(&PathBuf::from("/backup/a.jpg")));
    }

    #[test]
    fn test_group_selection_no_groups_shows_error() {
        // Files without group_name
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        app.enter_group_selection();
        // Should show error and stay in Reviewing mode
        assert!(app.error_message().is_some());
        assert_eq!(app.mode(), AppMode::Reviewing);
    }

    #[test]
    fn test_group_selection_prevents_selecting_all() {
        // All files in the group have the same group_name
        let groups = vec![make_group_with_names(
            100,
            vec![("/a.txt", Some("backup")), ("/b.txt", Some("backup"))],
        )];
        let mut app = App::with_groups(groups);

        app.enter_group_selection();
        app.select_by_group_name();

        // Should have set an error and NOT selected anything
        assert!(app.error_message().is_some());
        assert_eq!(app.selected_count(), 0);
    }

    #[test]
    fn test_select_all_by_group_name() {
        let groups = vec![
            make_group_with_names(
                100,
                vec![
                    ("/photos/a.jpg", Some("photos")),
                    ("/docs/a.jpg", Some("docs")),
                ],
            ),
            make_group_with_names(
                200,
                vec![
                    ("/photos/b.jpg", Some("photos")),
                    ("/backup/b.jpg", Some("backup")),
                ],
            ),
        ];
        let mut app = App::with_groups(groups);

        app.select_all_by_group_name("photos");

        // Should select all files with group_name "photos" across all groups
        assert!(app.is_file_selected(&PathBuf::from("/photos/a.jpg")));
        assert!(app.is_file_selected(&PathBuf::from("/photos/b.jpg")));
        // Other groups shouldn't be selected
        assert!(!app.is_file_selected(&PathBuf::from("/docs/a.jpg")));
        assert!(!app.is_file_selected(&PathBuf::from("/backup/b.jpg")));
        assert_eq!(app.selected_count(), 2);
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
        assert!(!app.groups()[0]
            .files
            .iter()
            .any(|f| f.path == PathBuf::from("/b.txt")));

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
        app.handle_action(Action::ToggleExpandAll);

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

        // App::with_groups sorts by size descending by default, so 200 is first
        let group = app.current_group().unwrap();
        assert_eq!(group.size, 200);

        app.next_group();
        let group = app.current_group().unwrap();
        assert_eq!(group.size, 100);
    }

    #[test]
    fn test_undo_selection() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt", "/c.txt"])];
        let mut app = App::with_groups(groups);

        app.select(PathBuf::from("/a.txt"));
        assert_eq!(app.selected_count(), 1);

        app.push_selection_history();
        app.select(PathBuf::from("/b.txt"));
        assert_eq!(app.selected_count(), 2);

        app.undo_selection();
        assert_eq!(app.selected_count(), 1);
        assert!(app.is_file_selected(&PathBuf::from("/a.txt")));
        assert!(!app.is_file_selected(&PathBuf::from("/b.txt")));
    }

    #[test]
    fn test_select_by_extension() {
        let groups = vec![
            make_group(100, vec!["/a.jpg", "/b.jpg"]),
            make_group(200, vec!["/c.png", "/d.png"]),
        ];
        let mut app = App::with_groups(groups);

        app.set_input_query("jpg".to_string());
        app.prepare_select_by_extension();

        assert_eq!(app.mode(), AppMode::ConfirmingBulkSelection);
        assert_eq!(app.pending_selection_count(), 1); // Kept /a.jpg, selected /b.jpg
        assert_eq!(
            app.pending_bulk_action(),
            Some(BulkSelectionType::ByExtension)
        );

        app.apply_bulk_selection();
        assert_eq!(app.mode(), AppMode::Reviewing);
        assert_eq!(app.selected_count(), 1);
        assert!(app.is_file_selected(&PathBuf::from("/b.jpg")));
    }

    #[test]
    fn test_select_by_directory() {
        let groups = vec![
            make_group(100, vec!["/dir1/a.txt", "/dir2/a.txt"]),
            make_group(200, vec!["/dir1/b.txt", "/dir2/b.txt"]),
        ];
        let mut app = App::with_groups(groups);

        app.set_input_query("/dir2".to_string());
        app.prepare_select_by_directory();

        assert_eq!(app.mode(), AppMode::ConfirmingBulkSelection);
        assert_eq!(app.pending_selection_count(), 2);

        app.apply_bulk_selection();
        assert_eq!(app.selected_count(), 2);
        assert!(app.is_file_selected(&PathBuf::from("/dir2/a.txt")));
        assert!(app.is_file_selected(&PathBuf::from("/dir2/b.txt")));
    }

    #[test]
    fn test_bulk_selection_keeps_one() {
        // Test that select_by_extension doesn't select ALL files in a group
        let groups = vec![make_group(100, vec!["/a.jpg", "/b.jpg", "/c.jpg"])];
        let mut app = App::with_groups(groups);

        app.set_input_query("jpg".to_string());
        app.prepare_select_by_extension();

        // Should select all but the first one
        assert_eq!(app.pending_selection_count(), 2);
        assert!(!app.pending_selections.contains(&PathBuf::from("/a.jpg")));
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
        app.handle_action(Action::ToggleExpandAll);

        assert!(app.handle_action(Action::NavigateDown));
        assert_eq!(app.file_index(), 1);

        assert!(app.handle_action(Action::NavigateUp));
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_handle_action_toggle_select() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups);

        // Initially collapsed
        assert!(!app.is_current_group_expanded());

        assert!(app.handle_action(Action::ToggleSelect));
        // First press should expand
        assert!(app.is_current_group_expanded());
        assert!(!app.is_current_selected());

        // Pressing space on first file (index 0) should collapse
        assert!(app.handle_action(Action::ToggleSelect));
        assert!(!app.is_current_group_expanded());

        // Expand again and move to second file
        app.handle_action(Action::ToggleExpandAll);
        app.next();
        assert_eq!(app.file_index(), 1);

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
    fn test_dry_run_blocks_delete() {
        let groups = vec![make_group(100, vec!["/a.txt", "/b.txt"])];
        let mut app = App::with_groups(groups).with_dry_run(true);

        app.toggle_select();
        assert!(app.has_selections());

        // Action::Delete should be blocked and return true (handled with error)
        assert!(app.handle_action(Action::Delete));
        assert_eq!(app.mode(), AppMode::Reviewing);
        assert!(app.error_message().is_some());
        assert!(app.error_message().unwrap().contains("dry-run"));
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
    fn test_apply_session_validation() {
        let groups = vec![
            make_group(100, vec!["/a.txt", "/b.txt"]),
            make_group(200, vec!["/c.txt", "/d.txt"]),
        ];
        let mut app = App::with_groups(groups);

        // Valid session
        let mut selections = std::collections::BTreeSet::new();
        selections.insert(PathBuf::from("/b.txt"));
        app.apply_session(selections, 1, 1);
        assert_eq!(app.group_index(), 1);
        assert_eq!(app.file_index(), 1);
        assert!(app.is_file_selected(&PathBuf::from("/b.txt")));

        // Invalid group_index
        app.apply_session(std::collections::BTreeSet::new(), 5, 0);
        assert_eq!(app.group_index(), 0);
        assert_eq!(app.file_index(), 0);

        // Invalid file_index
        app.apply_session(std::collections::BTreeSet::new(), 0, 10);
        assert_eq!(app.group_index(), 0);
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_go_to_top() {
        let groups = vec![make_group(
            100,
            vec!["/a.txt", "/b.txt", "/c.txt", "/d.txt"],
        )];
        let mut app = App::with_groups(groups);
        app.handle_action(Action::ToggleExpandAll);

        // Move to the middle
        app.next();
        app.next();
        assert_eq!(app.file_index(), 2);

        // Go to top
        app.go_to_top();
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_go_to_bottom() {
        let groups = vec![make_group(
            100,
            vec!["/a.txt", "/b.txt", "/c.txt", "/d.txt"],
        )];
        let mut app = App::with_groups(groups);

        assert_eq!(app.file_index(), 0);

        // Go to bottom
        app.go_to_bottom();
        assert_eq!(app.file_index(), 3);
    }

    #[test]
    fn test_handle_action_go_to_top_bottom() {
        let groups = vec![make_group(
            100,
            vec!["/a.txt", "/b.txt", "/c.txt", "/d.txt"],
        )];
        let mut app = App::with_groups(groups);

        // Go to bottom via action
        assert!(app.handle_action(Action::GoToBottom));
        assert_eq!(app.file_index(), 3);

        // Go to top via action
        assert!(app.handle_action(Action::GoToTop));
        assert_eq!(app.file_index(), 0);
    }

    #[test]
    fn test_go_to_top_bottom_in_folder_selection() {
        let groups = vec![make_group(
            100,
            vec!["/dir1/a.txt", "/dir2/b.txt", "/dir3/c.txt"],
        )];
        let mut app = App::with_groups(groups);

        // Enter folder selection mode
        app.enter_folder_selection();
        assert_eq!(app.mode(), AppMode::SelectingFolder);
        assert_eq!(app.folder_list().len(), 3);

        // Go to bottom
        app.go_to_bottom();
        assert_eq!(app.folder_index(), 2);

        // Go to top
        app.go_to_top();
        assert_eq!(app.folder_index(), 0);
    }

    #[test]
    fn test_search_logic() {
        let groups = vec![
            make_group(100, vec!["/photos/cat.jpg", "/backup/cat.jpg"]),
            make_group(200, vec!["/docs/work.pdf", "/old/work.pdf"]),
            make_group(300, vec!["/photos/dog.png", "/temp/dog.png"]),
        ];
        let mut app = App::with_groups(groups);

        assert_eq!(app.visible_group_count(), 3);

        // Search for "cat"
        app.set_search_query("cat".to_string());
        assert!(app.is_searching());
        assert_eq!(app.visible_group_count(), 1);
        assert_eq!(app.visible_group_at(0).unwrap().size, 100);

        // Search for "photos"
        app.set_search_query("photos".to_string());
        assert_eq!(app.visible_group_count(), 2);

        // Search for something that doesn't exist
        app.set_search_query("zebra".to_string());
        assert_eq!(app.visible_group_count(), 0);

        // Clear search
        app.clear_search();
        assert!(!app.is_searching());
        assert_eq!(app.visible_group_count(), 3);

        // Regex search
        app.set_search_query(".*\\.png".to_string());
        assert_eq!(app.visible_group_count(), 1);
        assert_eq!(app.visible_group_at(0).unwrap().size, 300);
    }

    #[test]
    fn test_navigation_with_search() {
        let groups = vec![
            make_group(100, vec!["/a/cat.jpg", "/b/cat.jpg"]),
            make_group(200, vec!["/a/dog.jpg", "/b/dog.jpg"]),
            make_group(300, vec!["/a/bird.jpg", "/b/bird.jpg"]),
        ];
        let mut app = App::with_groups(groups);

        // Filter to cat and bird
        app.set_search_query("cat".to_string());
        // Wait, "cat" only matches one group. Let's use ".jpg" which matches all, or "a/" which matches all.
        app.set_search_query("cat".to_string());
        assert_eq!(app.visible_group_count(), 1);
        assert_eq!(app.group_index(), 0);

        app.set_search_query("".to_string());
        assert_eq!(app.visible_group_count(), 3);

        // Match cat and bird
        // Since my apply_search is simple substring, I can't easily match cat OR bird with one query yet.
        // But I can match "jpg" which matches all.
        app.set_search_query("jpg".to_string());
        assert_eq!(app.visible_group_count(), 3);

        app.next_group();
        assert_eq!(app.group_index(), 1);
        assert_eq!(app.current_group().unwrap().size, 200);

        app.set_search_query("bird".to_string());
        assert_eq!(app.visible_group_count(), 1);
        assert_eq!(app.group_index(), 0); // Reset to 0 on new search
        assert_eq!(app.current_group().unwrap().size, 300);
    }

    #[test]
    fn test_sorting_groups() {
        let groups = vec![
            make_group(100, vec!["/z.txt", "/z2.txt"]),
            make_group(300, vec!["/a.txt", "/a2.txt"]),
            make_group(200, vec!["/m.txt", "/m2.txt", "/m3.txt"]),
        ];
        let mut app = App::with_groups(groups);

        // Default sort is Size Descending
        assert_eq!(app.sort_column(), SortColumn::Size);
        assert_eq!(app.sort_direction(), SortDirection::Descending);
        assert_eq!(app.groups()[0].size, 300);
        assert_eq!(app.groups()[1].size, 200);
        assert_eq!(app.groups()[2].size, 100);

        // Reverse to Ascending
        app.handle_action(Action::ReverseSortDirection);
        assert_eq!(app.sort_direction(), SortDirection::Ascending);
        assert_eq!(app.groups()[0].size, 100);
        assert_eq!(app.groups()[1].size, 200);
        assert_eq!(app.groups()[2].size, 300);

        // Cycle to Path
        app.handle_action(Action::CycleSortColumn);
        assert_eq!(app.sort_column(), SortColumn::Path);
        // path sort uses Descending by default (Wait, no, it uses whatever was current)
        // Let's reset to Ascending for path
        if app.sort_direction() == SortDirection::Descending {
            app.handle_action(Action::ReverseSortDirection);
        }
        assert_eq!(app.groups()[0].files[0].path, PathBuf::from("/a.txt"));
        assert_eq!(app.groups()[1].files[0].path, PathBuf::from("/m.txt"));
        assert_eq!(app.groups()[2].files[0].path, PathBuf::from("/z.txt"));

        // Cycle to Count
        app.handle_action(Action::CycleSortColumn); // Date
        app.handle_action(Action::CycleSortColumn); // Count
        assert_eq!(app.sort_column(), SortColumn::Count);
        app.handle_action(Action::ReverseSortDirection); // Descending
        assert_eq!(app.groups()[0].files.len(), 3); // 3 copies
        assert_eq!(app.groups()[1].files.len(), 2);
    }

    #[test]
    fn test_sorting_maintains_selection() {
        let groups = vec![
            make_group(100, vec!["/z.txt", "/z2.txt"]),
            make_group(300, vec!["/a.txt", "/a2.txt"]),
        ];
        let mut app = App::with_groups(groups);

        // Initial sort: 300, 100. Select group 0 (size 300)
        assert_eq!(app.current_group().unwrap().size, 300);
        assert_eq!(app.group_index(), 0);

        // Change sort to size ascending: 100, 300. Group 300 should now be at index 1
        app.handle_action(Action::ReverseSortDirection);
        assert_eq!(app.groups()[0].size, 100);
        assert_eq!(app.groups()[1].size, 300);
        assert_eq!(app.group_index(), 1);
        assert_eq!(app.current_group().unwrap().size, 300);
    }

    // =========================================================================
    // Action Parsing and Display Tests
    // =========================================================================

    #[test]
    fn test_action_name() {
        assert_eq!(Action::NavigateDown.name(), "navigate_down");
        assert_eq!(Action::GoToTop.name(), "go_to_top");
        assert_eq!(Action::Quit.name(), "quit");
        assert_eq!(Action::ToggleSelect.name(), "toggle_select");
    }

    #[test]
    fn test_action_display() {
        assert_eq!(Action::NavigateDown.to_string(), "navigate_down");
        assert_eq!(Action::GoToTop.to_string(), "go_to_top");
    }

    #[test]
    fn test_action_from_str_standard() {
        assert_eq!(
            "navigate_down".parse::<Action>().unwrap(),
            Action::NavigateDown
        );
        assert_eq!("navigate_up".parse::<Action>().unwrap(), Action::NavigateUp);
        assert_eq!("quit".parse::<Action>().unwrap(), Action::Quit);
        assert_eq!(
            "toggle_select".parse::<Action>().unwrap(),
            Action::ToggleSelect
        );
    }

    #[test]
    fn test_action_from_str_aliases() {
        // Short aliases
        assert_eq!("down".parse::<Action>().unwrap(), Action::NavigateDown);
        assert_eq!("up".parse::<Action>().unwrap(), Action::NavigateUp);
        assert_eq!("exit".parse::<Action>().unwrap(), Action::Quit);
        assert_eq!("select".parse::<Action>().unwrap(), Action::ToggleSelect);
        assert_eq!("esc".parse::<Action>().unwrap(), Action::Cancel);
        assert_eq!("enter".parse::<Action>().unwrap(), Action::Confirm);
    }

    #[test]
    fn test_action_from_str_case_insensitive() {
        assert_eq!(
            "NAVIGATE_DOWN".parse::<Action>().unwrap(),
            Action::NavigateDown
        );
        assert_eq!("Quit".parse::<Action>().unwrap(), Action::Quit);
    }

    #[test]
    fn test_action_from_str_hyphen_underscore() {
        assert_eq!(
            "navigate-down".parse::<Action>().unwrap(),
            Action::NavigateDown
        );
        assert_eq!("go-to-top".parse::<Action>().unwrap(), Action::GoToTop);
    }

    #[test]
    fn test_action_from_str_invalid() {
        let result = "invalid_action".parse::<Action>();
        assert!(result.is_err());
    }

    #[test]
    fn test_action_all_names() {
        let names = Action::all_names();
        assert_eq!(names.len(), 33);
        assert!(names.contains(&"navigate_down"));
        assert!(names.contains(&"show_help"));
        assert!(names.contains(&"select_group"));
        assert!(names.contains(&"search"));
        assert!(names.contains(&"quit"));
    }

    #[test]
    fn test_action_all() {
        let actions = Action::all();
        assert_eq!(actions.len(), 33);
        assert!(actions.contains(&Action::NavigateDown));
        assert!(actions.contains(&Action::ShowHelp));
        assert!(actions.contains(&Action::SelectGroup));
        assert!(actions.contains(&Action::Search));
        assert!(actions.contains(&Action::Quit));
    }
}
