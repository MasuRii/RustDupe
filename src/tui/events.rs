//! TUI event handling with crossterm.
//!
//! # Overview
//!
//! This module handles keyboard input and translates it to actions.
//! It provides both blocking and non-blocking event reading for the TUI main loop.
//!
//! # Key Mappings
//!
//! The default keybinding profile (Universal) supports both vim-style and arrow key
//! navigation simultaneously. See [`KeyBindings`] for profile-specific mappings.
//!
//! | Key | Action |
//! |-----|--------|
//! | `j` / `Down` | Navigate to next file |
//! | `k` / `Up` | Navigate to previous file |
//! | `h` / `Left` | Navigate to previous group (back) |
//! | `l` / `Right` | Navigate to next group (forward) |
//! | `J` / `Page Down` / `Ctrl+d` | Navigate to next group |
//! | `K` / `Page Up` / `Ctrl+u` | Navigate to previous group |
//! | `g` / `Home` | Go to top of list |
//! | `G` / `End` | Go to bottom of list |
//! | `Space` | Toggle selection of current file |
//! | `a` | Select all in current group (except first) |
//! | `u` | Deselect all files |
//! | `Enter` | Confirm current action |
//! | `Escape` | Cancel current action |
//! | `p` | Preview current file |
//! | `d` | Delete selected files (to trash) |
//! | `q` | Quit application |
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::tui::events::EventHandler;
//! use std::time::Duration;
//!
//! let event_handler = EventHandler::new();
//!
//! // Non-blocking poll with 100ms timeout
//! if let Some(action) = event_handler.poll(Duration::from_millis(100)).unwrap() {
//!     println!("Action received: {:?}", action);
//! }
//! ```

use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent};

use super::keybindings::{KeyBindings, KeybindingProfile};
use super::Action;

/// Event handler for the TUI.
///
/// Handles keyboard events and translates them to [`Action`]s using
/// configurable [`KeyBindings`]. Uses crossterm for cross-platform input handling.
///
/// # Thread Safety
///
/// This struct should only be used from the main thread.
/// Crossterm's event handling is not thread-safe.
#[derive(Debug, Clone)]
pub struct EventHandler {
    /// Keybindings configuration for translating key events to actions.
    bindings: KeyBindings,
}

/// Error type for event handling operations.
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    /// I/O error from crossterm
    #[error("Event I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHandler {
    /// Create a new event handler with default (Universal) keybindings.
    ///
    /// The Universal profile supports both vim-style (hjkl) AND arrow key
    /// navigation simultaneously.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::events::EventHandler;
    /// let handler = EventHandler::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: KeyBindings::default(),
        }
    }

    /// Create a new event handler with a specific keybinding profile.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::events::EventHandler;
    /// use rustdupe::tui::keybindings::KeybindingProfile;
    ///
    /// let handler = EventHandler::with_profile(KeybindingProfile::Vim);
    /// ```
    #[must_use]
    pub fn with_profile(profile: KeybindingProfile) -> Self {
        Self {
            bindings: KeyBindings::from_profile(profile),
        }
    }

    /// Create a new event handler with custom keybindings.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::events::EventHandler;
    /// use rustdupe::tui::keybindings::KeyBindings;
    ///
    /// let bindings = KeyBindings::default();
    /// let handler = EventHandler::with_bindings(bindings);
    /// ```
    #[must_use]
    pub fn with_bindings(bindings: KeyBindings) -> Self {
        Self { bindings }
    }

    /// Get a reference to the current keybindings.
    #[must_use]
    pub fn bindings(&self) -> &KeyBindings {
        &self.bindings
    }

    /// Poll for an event with the specified timeout.
    ///
    /// Returns `Ok(Some(action))` if an action was triggered,
    /// `Ok(None)` if no event occurred within the timeout,
    /// or `Err` on I/O error.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for an event
    ///
    /// # Errors
    ///
    /// Returns `EventError::Io` if there's an I/O error reading events.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::tui::events::EventHandler;
    /// use std::time::Duration;
    ///
    /// let handler = EventHandler::new();
    /// match handler.poll(Duration::from_millis(100)) {
    ///     Ok(Some(action)) => println!("Action: {:?}", action),
    ///     Ok(None) => println!("No event"),
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn poll(&self, timeout: Duration) -> Result<Option<Action>, EventError> {
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                return Ok(self.translate_key(key));
            }
            // Ignore non-key events (mouse, resize, etc.)
        }
        Ok(None)
    }

    /// Read an event, blocking until one is available.
    ///
    /// Returns `Ok(Some(action))` if an action was triggered,
    /// `Ok(None)` if the event was not a mapped key,
    /// or `Err` on I/O error.
    ///
    /// # Errors
    ///
    /// Returns `EventError::Io` if there's an I/O error reading events.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::tui::events::EventHandler;
    ///
    /// let handler = EventHandler::new();
    /// if let Ok(Some(action)) = handler.read() {
    ///     println!("Action: {:?}", action);
    /// }
    /// ```
    pub fn read(&self) -> Result<Option<Action>, EventError> {
        let event = event::read()?;
        if let Event::Key(key) = event {
            return Ok(self.translate_key(key));
        }
        // Ignore non-key events
        Ok(None)
    }

    /// Check if there's an event available without blocking.
    ///
    /// # Errors
    ///
    /// Returns `EventError::Io` if there's an I/O error.
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::tui::events::EventHandler;
    /// let handler = EventHandler::new();
    /// if handler.has_event().unwrap() {
    ///     // Process event
    /// }
    /// ```
    pub fn has_event(&self) -> Result<bool, EventError> {
        Ok(event::poll(Duration::ZERO)?)
    }

    /// Translate a key event to an action using the configured keybindings.
    ///
    /// Returns `None` if the key is not mapped to any action.
    fn translate_key(&self, key: KeyEvent) -> Option<Action> {
        self.bindings.resolve(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_release_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_event_handler_new() {
        let handler = EventHandler::new();
        // Verify construction works and uses Universal profile
        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Quit));
    }

    #[test]
    fn test_event_handler_with_profile() {
        let handler = EventHandler::with_profile(KeybindingProfile::Vim);
        // Vim profile should still resolve 'j' to NavigateDown
        let key = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NavigateDown));

        // But arrow keys should NOT work in Vim profile
        let key = make_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), None);
    }

    #[test]
    fn test_event_handler_bindings_accessor() {
        let handler = EventHandler::new();
        let bindings = handler.bindings();
        assert_eq!(bindings.profile(), KeybindingProfile::Universal);
    }

    #[test]
    fn test_translate_navigate_down() {
        let handler = EventHandler::new();

        // 'j' key
        let key = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NavigateDown));

        // Down arrow
        let key = make_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_translate_navigate_up() {
        let handler = EventHandler::new();

        // 'k' key
        let key = make_key(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NavigateUp));

        // Up arrow
        let key = make_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NavigateUp));
    }

    #[test]
    fn test_translate_next_group() {
        let handler = EventHandler::new();

        // 'J' with shift
        let key = make_key(KeyCode::Char('J'), KeyModifiers::SHIFT);
        assert_eq!(handler.translate_key(key), Some(Action::NextGroup));

        // Page Down
        let key = make_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NextGroup));

        // Ctrl+d
        let key = make_key(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(handler.translate_key(key), Some(Action::NextGroup));

        // Right arrow (forward navigation)
        let key = make_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NextGroup));
    }

    #[test]
    fn test_translate_previous_group() {
        let handler = EventHandler::new();

        // 'K' with shift
        let key = make_key(KeyCode::Char('K'), KeyModifiers::SHIFT);
        assert_eq!(handler.translate_key(key), Some(Action::PreviousGroup));

        // Page Up
        let key = make_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::PreviousGroup));

        // Ctrl+u
        let key = make_key(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(handler.translate_key(key), Some(Action::PreviousGroup));

        // 'h' key (vim left / back)
        let key = make_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::PreviousGroup));

        // Left arrow (back navigation)
        let key = make_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::PreviousGroup));
    }

    #[test]
    fn test_translate_go_to_top() {
        let handler = EventHandler::new();

        // Home key
        let key = make_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::GoToTop));

        // 'g' key (vim go to top)
        let key = make_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::GoToTop));
    }

    #[test]
    fn test_translate_go_to_bottom() {
        let handler = EventHandler::new();

        // End key
        let key = make_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::GoToBottom));

        // 'G' key (vim go to bottom)
        let key = make_key(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(handler.translate_key(key), Some(Action::GoToBottom));
    }

    #[test]
    fn test_translate_toggle_select() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::ToggleSelect));
    }

    #[test]
    fn test_translate_select_all_in_group() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::SelectAllInGroup));
    }

    #[test]
    fn test_translate_deselect_all() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Char('u'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::DeselectAll));
    }

    #[test]
    fn test_translate_batch_selections() {
        let handler = EventHandler::new();

        // All duplicates
        let key = make_key(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(
            handler.translate_key(key),
            Some(Action::SelectAllDuplicates)
        );

        // Oldest
        let key = make_key(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::SelectOldest));
        let key = make_key(KeyCode::Char('O'), KeyModifiers::SHIFT);
        assert_eq!(handler.translate_key(key), Some(Action::SelectOldest));

        // Newest
        let key = make_key(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::SelectNewest));
        let key = make_key(KeyCode::Char('N'), KeyModifiers::SHIFT);
        assert_eq!(handler.translate_key(key), Some(Action::SelectNewest));

        // Size
        let key = make_key(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::SelectSmallest));
        let key = make_key(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::SelectLargest));
    }

    #[test]
    fn test_translate_confirm() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Confirm));
    }

    #[test]
    fn test_translate_cancel() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Cancel));
    }

    #[test]
    fn test_translate_preview() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Preview));
    }

    #[test]
    fn test_translate_delete() {
        let handler = EventHandler::new();

        let key = make_key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Delete));
    }

    #[test]
    fn test_translate_quit() {
        let handler = EventHandler::new();

        // 'q' key
        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Quit));

        // Ctrl+C
        let key = make_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(handler.translate_key(key), Some(Action::Quit));
    }

    #[test]
    fn test_translate_unmapped_key() {
        let handler = EventHandler::new();

        // Random letter that's not mapped
        let key = make_key(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), None);

        // Tab
        let key = make_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), None);
    }

    #[test]
    fn test_translate_show_help() {
        let handler = EventHandler::new();

        // '?' key
        let key = make_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::ShowHelp));

        // F1 key
        let key = make_key(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::ShowHelp));
    }

    #[test]
    fn test_ignore_key_release() {
        let handler = EventHandler::new();

        // Key release should be ignored
        let key = make_release_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), None);
    }

    #[test]
    fn test_modifier_keys_matter() {
        let handler = EventHandler::new();

        // 'j' without modifiers = NavigateDown
        let key = make_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::NavigateDown));

        // Alt+J = unmapped (should not trigger NavigateDown)
        let key = make_key(KeyCode::Char('j'), KeyModifiers::ALT);
        assert_eq!(handler.translate_key(key), None);
    }

    #[test]
    fn test_event_error_display() {
        let io_error = std::io::Error::other("test error");
        let event_error = EventError::Io(io_error);
        let display = format!("{}", event_error);
        assert!(display.contains("Event I/O error"));
    }

    #[test]
    fn test_default_implementation() {
        let handler = EventHandler::default();
        let key = make_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(handler.translate_key(key), Some(Action::Quit));
    }

    #[test]
    fn test_dual_navigation_vim_and_arrows() {
        // Verify that Universal profile supports BOTH vim and arrows
        let handler = EventHandler::new();

        // vim style
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Char('j'), KeyModifiers::NONE)),
            Some(Action::NavigateDown)
        );
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Char('k'), KeyModifiers::NONE)),
            Some(Action::NavigateUp)
        );
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Char('h'), KeyModifiers::NONE)),
            Some(Action::PreviousGroup)
        );

        // arrow style
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Down, KeyModifiers::NONE)),
            Some(Action::NavigateDown)
        );
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Up, KeyModifiers::NONE)),
            Some(Action::NavigateUp)
        );
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Left, KeyModifiers::NONE)),
            Some(Action::PreviousGroup)
        );
        assert_eq!(
            handler.translate_key(make_key(KeyCode::Right, KeyModifiers::NONE)),
            Some(Action::NextGroup)
        );
    }
}
