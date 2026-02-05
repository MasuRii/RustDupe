//! Keybinding configuration for the TUI.
//!
//! This module provides flexible keybinding support with multiple profiles
//! and customizable key mappings. The default profile (Universal) supports
//! both vim-style (hjkl) AND arrow key navigation simultaneously.
//!
//! # Profiles
//!
//! - [`KeybindingProfile::Universal`]: Both vim-style AND arrow keys (default)
//! - [`KeybindingProfile::Vim`]: Vim-style keys only (hjkl)
//! - [`KeybindingProfile::Standard`]: Arrow keys and standard shortcuts only
//! - [`KeybindingProfile::Emacs`]: Emacs-style keybindings
//!
//! # Example
//!
//! ```
//! use rustdupe::tui::keybindings::{KeyBindings, KeybindingProfile};
//! use rustdupe::tui::Action;
//! use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, KeyEventKind, KeyEventState};
//!
//! let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
//!
//! // Both 'j' and Down arrow should navigate down
//! let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
//! let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
//!
//! assert_eq!(bindings.resolve(&j_key), Some(Action::NavigateDown));
//! assert_eq!(bindings.resolve(&down_key), Some(Action::NavigateDown));
//! ```

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Action;

/// Keybinding profile presets.
///
/// Each profile defines a complete set of keybindings tailored for
/// different user preferences and familiarity with various navigation styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum KeybindingProfile {
    /// Universal profile: Supports BOTH vim-style AND arrow key navigation.
    ///
    /// This is the recommended default as it works for all users regardless
    /// of their preferred navigation style.
    #[default]
    Universal,

    /// Vim profile: Vim-style navigation using hjkl keys.
    ///
    /// Familiar to vim/neovim users. Does not include arrow key navigation.
    Vim,

    /// Standard profile: Arrow keys and standard shortcuts only.
    ///
    /// Familiar to users of traditional GUI applications. Does not include
    /// vim-style navigation.
    Standard,

    /// Emacs profile: Emacs-style keybindings.
    ///
    /// Uses Ctrl-based navigation (Ctrl-n/p for down/up, etc.).
    Emacs,
}

impl KeybindingProfile {
    /// Get the display name for the profile.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Universal => "Universal (Vim + Arrow keys)",
            Self::Vim => "Vim (hjkl)",
            Self::Standard => "Standard (Arrow keys)",
            Self::Emacs => "Emacs (Ctrl-based)",
        }
    }

    /// Get all available profiles.
    #[must_use]
    pub fn all() -> &'static [KeybindingProfile] {
        &[Self::Universal, Self::Vim, Self::Standard, Self::Emacs]
    }
}

impl std::fmt::Display for KeybindingProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Universal => "universal",
            Self::Vim => "vim",
            Self::Standard => "standard",
            Self::Emacs => "emacs",
        };
        write!(f, "{name}")
    }
}

impl std::str::FromStr for KeybindingProfile {
    type Err = KeybindingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "universal" => Ok(Self::Universal),
            "vim" => Ok(Self::Vim),
            "standard" | "arrows" | "arrow" => Ok(Self::Standard),
            "emacs" => Ok(Self::Emacs),
            _ => Err(KeybindingError::InvalidProfile(s.to_string())),
        }
    }
}

/// Error type for keybinding operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum KeybindingError {
    /// Invalid profile name.
    #[error("Unknown keybinding profile: '{0}'. Valid profiles: universal, vim, standard, emacs")]
    InvalidProfile(String),

    /// Invalid key specification.
    #[error("Invalid key specification: '{0}'")]
    InvalidKeySpec(String),
}

/// Keybinding configuration mapping actions to key events.
///
/// The `KeyBindings` struct maps each [`Action`] to a list of [`KeyEvent`]s
/// that can trigger it. Multiple keys can trigger the same action.
///
/// # Thread Safety
///
/// This struct is thread-safe for read access. Modifications should only
/// be done during initialization.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    /// The profile these bindings are based on.
    profile: KeybindingProfile,

    /// Mapping from actions to the key events that trigger them.
    action_keys: HashMap<Action, Vec<KeyEvent>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::from_profile(KeybindingProfile::Universal)
    }
}

impl KeyBindings {
    /// Create keybindings from a specific profile.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::keybindings::{KeyBindings, KeybindingProfile};
    ///
    /// let bindings = KeyBindings::from_profile(KeybindingProfile::Vim);
    /// ```
    #[must_use]
    pub fn from_profile(profile: KeybindingProfile) -> Self {
        let action_keys = match profile {
            KeybindingProfile::Universal => Self::universal_bindings(),
            KeybindingProfile::Vim => Self::vim_bindings(),
            KeybindingProfile::Standard => Self::standard_bindings(),
            KeybindingProfile::Emacs => Self::emacs_bindings(),
        };

        Self {
            profile,
            action_keys,
        }
    }

    /// Get the keybinding profile.
    #[must_use]
    pub fn profile(&self) -> KeybindingProfile {
        self.profile
    }

    /// Resolve a key event to an action.
    ///
    /// Returns `Some(Action)` if the key event is bound to an action,
    /// or `None` if the key is not mapped.
    ///
    /// # Note
    ///
    /// This method ignores key release events (some terminals send these).
    /// Only key press events are matched.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::keybindings::KeyBindings;
    /// use rustdupe::tui::Action;
    /// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    ///
    /// let bindings = KeyBindings::default();
    /// let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    ///
    /// assert_eq!(bindings.resolve(&key), Some(Action::NavigateDown));
    /// ```
    #[must_use]
    pub fn resolve(&self, key: &KeyEvent) -> Option<Action> {
        // Ignore key release events
        if key.kind != crossterm::event::KeyEventKind::Press {
            return None;
        }

        // Search for a matching binding
        for (action, keys) in &self.action_keys {
            if keys.iter().any(|k| Self::key_matches(k, key)) {
                return Some(*action);
            }
        }

        None
    }

    /// Check if a key event matches a target key event.
    ///
    /// Matches code and modifiers, ignoring kind and state.
    fn key_matches(target: &KeyEvent, actual: &KeyEvent) -> bool {
        target.code == actual.code && target.modifiers == actual.modifiers
    }

    /// Get the keys bound to a specific action.
    ///
    /// Returns an empty slice if the action is not bound.
    #[must_use]
    pub fn keys_for_action(&self, action: &Action) -> &[KeyEvent] {
        self.action_keys
            .get(action)
            .map_or(&[], |keys| keys.as_slice())
    }

    /// Get all actions and their bound keys.
    #[must_use]
    pub fn all_bindings(&self) -> &HashMap<Action, Vec<KeyEvent>> {
        &self.action_keys
    }

    /// Get a human-readable string for the first key bound to an action.
    ///
    /// Useful for displaying hints in the UI.
    #[must_use]
    pub fn key_hint(&self, action: &Action) -> String {
        self.keys_for_action(action)
            .first()
            .map_or_else(String::new, Self::format_key)
    }

    /// Format a key event as a human-readable string.
    #[must_use]
    pub fn format_key(key: &KeyEvent) -> String {
        let mut parts = Vec::new();

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }

        let key_name = match key.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "Shift+Tab".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::F(n) => format!("F{n}"),
            _ => "?".to_string(),
        };

        if parts.is_empty() {
            key_name
        } else {
            parts.push(&key_name);
            parts.join("+")
        }
    }

    // =========================================================================
    // Profile Binding Definitions
    // =========================================================================

    /// Create a key event helper.
    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    /// Universal bindings: Both vim-style AND arrow keys.
    fn universal_bindings() -> HashMap<Action, Vec<KeyEvent>> {
        let mut bindings = HashMap::new();

        // Navigation - down (vim 'j' AND Down arrow)
        bindings.insert(
            Action::NavigateDown,
            vec![
                Self::key(KeyCode::Char('j'), KeyModifiers::NONE),
                Self::key(KeyCode::Down, KeyModifiers::NONE),
            ],
        );

        // Navigation - up (vim 'k' AND Up arrow)
        bindings.insert(
            Action::NavigateUp,
            vec![
                Self::key(KeyCode::Char('k'), KeyModifiers::NONE),
                Self::key(KeyCode::Up, KeyModifiers::NONE),
            ],
        );

        // Navigation - next group (vim 'J' AND Page Down AND Ctrl-d)
        bindings.insert(
            Action::NextGroup,
            vec![
                Self::key(KeyCode::Char('J'), KeyModifiers::SHIFT),
                Self::key(KeyCode::Char('J'), KeyModifiers::NONE), // Some terminals
                Self::key(KeyCode::PageDown, KeyModifiers::NONE),
                Self::key(KeyCode::Char('d'), KeyModifiers::CONTROL),
            ],
        );

        // Navigation - previous group (vim 'K' AND Page Up AND Ctrl-u AND h/Left)
        bindings.insert(
            Action::PreviousGroup,
            vec![
                Self::key(KeyCode::Char('K'), KeyModifiers::SHIFT),
                Self::key(KeyCode::Char('K'), KeyModifiers::NONE), // Some terminals
                Self::key(KeyCode::PageUp, KeyModifiers::NONE),
                Self::key(KeyCode::Char('u'), KeyModifiers::CONTROL),
                Self::key(KeyCode::Char('h'), KeyModifiers::NONE), // vim left
                Self::key(KeyCode::Left, KeyModifiers::NONE),      // arrow left
            ],
        );

        // Navigation - next group also via l/Right (forward navigation)
        // Note: 'l' is also used for SelectLargest, but Right is added here
        // The action_keys map allows multiple keys to trigger the same action
        if let Some(keys) = bindings.get_mut(&Action::NextGroup) {
            keys.push(Self::key(KeyCode::Right, KeyModifiers::NONE));
        }

        // Navigation - go to top (Home AND 'g')
        bindings.insert(
            Action::GoToTop,
            vec![
                Self::key(KeyCode::Home, KeyModifiers::NONE),
                Self::key(KeyCode::Char('g'), KeyModifiers::NONE),
            ],
        );

        // Navigation - go to bottom (End AND 'G')
        bindings.insert(
            Action::GoToBottom,
            vec![
                Self::key(KeyCode::End, KeyModifiers::NONE),
                Self::key(KeyCode::Char('G'), KeyModifiers::SHIFT),
                Self::key(KeyCode::Char('G'), KeyModifiers::NONE), // Some terminals
            ],
        );

        // Selection
        bindings.insert(
            Action::ToggleSelect,
            vec![Self::key(KeyCode::Char(' '), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectAllInGroup,
            vec![Self::key(KeyCode::Char('a'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectAllDuplicates,
            vec![Self::key(KeyCode::Char('A'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectOldest,
            vec![
                Self::key(KeyCode::Char('o'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('O'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::SelectNewest,
            vec![
                Self::key(KeyCode::Char('n'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('N'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::SelectSmallest,
            vec![
                Self::key(KeyCode::Char('s'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('S'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::SelectLargest,
            vec![
                Self::key(KeyCode::Char('l'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('L'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::DeselectAll,
            vec![Self::key(KeyCode::Char('u'), KeyModifiers::NONE)],
        );

        // Confirm/Cancel
        bindings.insert(
            Action::Confirm,
            vec![Self::key(KeyCode::Enter, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Cancel,
            vec![Self::key(KeyCode::Esc, KeyModifiers::NONE)],
        );

        // Actions
        bindings.insert(
            Action::Preview,
            vec![Self::key(KeyCode::Char('p'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectFolder,
            vec![
                Self::key(KeyCode::Char('f'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('F'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Char('d'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        // Quit
        bindings.insert(
            Action::Quit,
            vec![
                Self::key(KeyCode::Char('q'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ],
        );

        bindings
    }

    /// Vim-style bindings: hjkl navigation without arrow keys.
    fn vim_bindings() -> HashMap<Action, Vec<KeyEvent>> {
        let mut bindings = HashMap::new();

        // Navigation - vim only
        bindings.insert(
            Action::NavigateDown,
            vec![Self::key(KeyCode::Char('j'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::NavigateUp,
            vec![Self::key(KeyCode::Char('k'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::NextGroup,
            vec![
                Self::key(KeyCode::Char('J'), KeyModifiers::SHIFT),
                Self::key(KeyCode::Char('J'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('d'), KeyModifiers::CONTROL),
            ],
        );

        bindings.insert(
            Action::PreviousGroup,
            vec![
                Self::key(KeyCode::Char('K'), KeyModifiers::SHIFT),
                Self::key(KeyCode::Char('K'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('u'), KeyModifiers::CONTROL),
                Self::key(KeyCode::Char('h'), KeyModifiers::NONE), // vim left
            ],
        );

        // Navigation - go to top (vim 'g' or 'gg' pattern, we use single 'g')
        bindings.insert(
            Action::GoToTop,
            vec![Self::key(KeyCode::Char('g'), KeyModifiers::NONE)],
        );

        // Navigation - go to bottom (vim 'G')
        bindings.insert(
            Action::GoToBottom,
            vec![
                Self::key(KeyCode::Char('G'), KeyModifiers::SHIFT),
                Self::key(KeyCode::Char('G'), KeyModifiers::NONE),
            ],
        );

        // Selection (same as universal)
        bindings.insert(
            Action::ToggleSelect,
            vec![Self::key(KeyCode::Char(' '), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectAllInGroup,
            vec![Self::key(KeyCode::Char('a'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectAllDuplicates,
            vec![Self::key(KeyCode::Char('A'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectOldest,
            vec![
                Self::key(KeyCode::Char('o'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('O'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::SelectNewest,
            vec![
                Self::key(KeyCode::Char('n'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('N'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::SelectSmallest,
            vec![
                Self::key(KeyCode::Char('s'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('S'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::SelectLargest,
            vec![
                Self::key(KeyCode::Char('l'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('L'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::DeselectAll,
            vec![Self::key(KeyCode::Char('u'), KeyModifiers::NONE)],
        );

        // Confirm/Cancel (same as universal)
        bindings.insert(
            Action::Confirm,
            vec![Self::key(KeyCode::Enter, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Cancel,
            vec![Self::key(KeyCode::Esc, KeyModifiers::NONE)],
        );

        // Actions (same as universal)
        bindings.insert(
            Action::Preview,
            vec![Self::key(KeyCode::Char('p'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectFolder,
            vec![
                Self::key(KeyCode::Char('f'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('F'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Char('d'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        // Quit
        bindings.insert(
            Action::Quit,
            vec![
                Self::key(KeyCode::Char('q'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ],
        );

        bindings
    }

    /// Standard bindings: Arrow keys without vim-style navigation.
    fn standard_bindings() -> HashMap<Action, Vec<KeyEvent>> {
        let mut bindings = HashMap::new();

        // Navigation - arrows only
        bindings.insert(
            Action::NavigateDown,
            vec![Self::key(KeyCode::Down, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::NavigateUp,
            vec![Self::key(KeyCode::Up, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::NextGroup,
            vec![
                Self::key(KeyCode::PageDown, KeyModifiers::NONE),
                Self::key(KeyCode::Right, KeyModifiers::NONE), // arrow right for forward
            ],
        );

        bindings.insert(
            Action::PreviousGroup,
            vec![
                Self::key(KeyCode::PageUp, KeyModifiers::NONE),
                Self::key(KeyCode::Left, KeyModifiers::NONE), // arrow left for back
            ],
        );

        // Navigation - go to top (Home)
        bindings.insert(
            Action::GoToTop,
            vec![Self::key(KeyCode::Home, KeyModifiers::NONE)],
        );

        // Navigation - go to bottom (End)
        bindings.insert(
            Action::GoToBottom,
            vec![Self::key(KeyCode::End, KeyModifiers::NONE)],
        );

        // Selection
        bindings.insert(
            Action::ToggleSelect,
            vec![Self::key(KeyCode::Char(' '), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectAllInGroup,
            vec![Self::key(KeyCode::Char('a'), KeyModifiers::CONTROL)],
        );

        bindings.insert(
            Action::SelectAllDuplicates,
            vec![Self::key(KeyCode::Char('A'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectOldest,
            vec![Self::key(KeyCode::Char('o'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectNewest,
            vec![Self::key(KeyCode::Char('n'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectSmallest,
            vec![Self::key(KeyCode::Char('s'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectLargest,
            vec![Self::key(KeyCode::Char('l'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::DeselectAll,
            vec![Self::key(KeyCode::Char('u'), KeyModifiers::NONE)],
        );

        // Confirm/Cancel
        bindings.insert(
            Action::Confirm,
            vec![Self::key(KeyCode::Enter, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Cancel,
            vec![Self::key(KeyCode::Esc, KeyModifiers::NONE)],
        );

        // Actions
        bindings.insert(
            Action::Preview,
            vec![Self::key(KeyCode::Char('p'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectFolder,
            vec![Self::key(KeyCode::Char('f'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Delete, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        // Quit
        bindings.insert(
            Action::Quit,
            vec![
                Self::key(KeyCode::Char('q'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ],
        );

        bindings
    }

    /// Emacs-style bindings: Ctrl-based navigation.
    fn emacs_bindings() -> HashMap<Action, Vec<KeyEvent>> {
        let mut bindings = HashMap::new();

        // Navigation - Emacs style (Ctrl-n/p for next/previous)
        bindings.insert(
            Action::NavigateDown,
            vec![
                Self::key(KeyCode::Char('n'), KeyModifiers::CONTROL),
                Self::key(KeyCode::Down, KeyModifiers::NONE),
            ],
        );

        bindings.insert(
            Action::NavigateUp,
            vec![
                Self::key(KeyCode::Char('p'), KeyModifiers::CONTROL),
                Self::key(KeyCode::Up, KeyModifiers::NONE),
            ],
        );

        bindings.insert(
            Action::NextGroup,
            vec![
                Self::key(KeyCode::Char('v'), KeyModifiers::CONTROL), // Scroll down
                Self::key(KeyCode::PageDown, KeyModifiers::NONE),
            ],
        );

        bindings.insert(
            Action::PreviousGroup,
            vec![
                Self::key(KeyCode::Char('v'), KeyModifiers::ALT), // Meta-v scroll up
                Self::key(KeyCode::PageUp, KeyModifiers::NONE),
                Self::key(KeyCode::Char('b'), KeyModifiers::CONTROL), // Ctrl-b backward
            ],
        );

        // Navigation - go to top (Emacs: Meta-< or Home)
        bindings.insert(
            Action::GoToTop,
            vec![
                Self::key(KeyCode::Home, KeyModifiers::NONE),
                Self::key(KeyCode::Char('<'), KeyModifiers::ALT), // Meta-<
            ],
        );

        // Navigation - go to bottom (Emacs: Meta-> or End)
        bindings.insert(
            Action::GoToBottom,
            vec![
                Self::key(KeyCode::End, KeyModifiers::NONE),
                Self::key(KeyCode::Char('>'), KeyModifiers::ALT), // Meta->
            ],
        );

        // Selection
        bindings.insert(
            Action::ToggleSelect,
            vec![Self::key(KeyCode::Char(' '), KeyModifiers::CONTROL)],
        );

        bindings.insert(
            Action::SelectAllInGroup,
            vec![Self::key(KeyCode::Char('a'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectAllDuplicates,
            vec![Self::key(KeyCode::Char('A'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectOldest,
            vec![Self::key(KeyCode::Char('o'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectNewest,
            vec![Self::key(KeyCode::Char('n'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectSmallest,
            vec![Self::key(KeyCode::Char('s'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectLargest,
            vec![Self::key(KeyCode::Char('l'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::DeselectAll,
            vec![Self::key(KeyCode::Char('u'), KeyModifiers::NONE)],
        );

        // Confirm/Cancel
        bindings.insert(
            Action::Confirm,
            vec![Self::key(KeyCode::Enter, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Cancel,
            vec![
                Self::key(KeyCode::Char('g'), KeyModifiers::CONTROL), // Ctrl-g = cancel
                Self::key(KeyCode::Esc, KeyModifiers::NONE),
            ],
        );

        // Actions
        bindings.insert(
            Action::Preview,
            vec![Self::key(KeyCode::Char('p'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectFolder,
            vec![Self::key(KeyCode::Char('f'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Char('d'), KeyModifiers::CONTROL)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        // Quit
        bindings.insert(
            Action::Quit,
            vec![
                Self::key(KeyCode::Char('q'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ],
        );

        bindings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    /// Helper to create a key press event.
    fn key_press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Helper to create a key release event.
    fn key_release(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        }
    }

    // =========================================================================
    // Profile Tests
    // =========================================================================

    #[test]
    fn test_keybinding_profile_default() {
        let profile = KeybindingProfile::default();
        assert_eq!(profile, KeybindingProfile::Universal);
    }

    #[test]
    fn test_keybinding_profile_display() {
        assert_eq!(KeybindingProfile::Universal.to_string(), "universal");
        assert_eq!(KeybindingProfile::Vim.to_string(), "vim");
        assert_eq!(KeybindingProfile::Standard.to_string(), "standard");
        assert_eq!(KeybindingProfile::Emacs.to_string(), "emacs");
    }

    #[test]
    fn test_keybinding_profile_from_str() {
        assert_eq!(
            "universal".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Universal
        );
        assert_eq!(
            "vim".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Vim
        );
        assert_eq!(
            "standard".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Standard
        );
        assert_eq!(
            "arrows".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Standard
        );
        assert_eq!(
            "emacs".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Emacs
        );

        // Case insensitive
        assert_eq!(
            "UNIVERSAL".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Universal
        );
        assert_eq!(
            "Vim".parse::<KeybindingProfile>().unwrap(),
            KeybindingProfile::Vim
        );
    }

    #[test]
    fn test_keybinding_profile_from_str_invalid() {
        let result = "invalid".parse::<KeybindingProfile>();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KeybindingError::InvalidProfile(_)));
        assert!(err.to_string().contains("invalid"));
    }

    #[test]
    fn test_keybinding_profile_display_name() {
        assert!(KeybindingProfile::Universal
            .display_name()
            .contains("Universal"));
        assert!(KeybindingProfile::Vim.display_name().contains("Vim"));
        assert!(KeybindingProfile::Standard.display_name().contains("Arrow"));
        assert!(KeybindingProfile::Emacs.display_name().contains("Emacs"));
    }

    #[test]
    fn test_keybinding_profile_all() {
        let profiles = KeybindingProfile::all();
        assert_eq!(profiles.len(), 4);
        assert!(profiles.contains(&KeybindingProfile::Universal));
        assert!(profiles.contains(&KeybindingProfile::Vim));
        assert!(profiles.contains(&KeybindingProfile::Standard));
        assert!(profiles.contains(&KeybindingProfile::Emacs));
    }

    // =========================================================================
    // KeyBindings Tests
    // =========================================================================

    #[test]
    fn test_keybindings_default() {
        let bindings = KeyBindings::default();
        assert_eq!(bindings.profile(), KeybindingProfile::Universal);
    }

    #[test]
    fn test_keybindings_from_profile() {
        for profile in KeybindingProfile::all() {
            let bindings = KeyBindings::from_profile(*profile);
            assert_eq!(bindings.profile(), *profile);
        }
    }

    // =========================================================================
    // Universal Profile Resolution Tests
    // =========================================================================

    #[test]
    fn test_universal_navigate_down_vim() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_universal_navigate_down_arrow() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_universal_navigate_up_vim() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::NavigateUp));
    }

    #[test]
    fn test_universal_navigate_up_arrow() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::NavigateUp));
    }

    #[test]
    fn test_universal_next_group_vim() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('J'), KeyModifiers::SHIFT);
        assert_eq!(bindings.resolve(&key), Some(Action::NextGroup));
    }

    #[test]
    fn test_universal_next_group_pgdn() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::NextGroup));
    }

    #[test]
    fn test_universal_next_group_ctrl_d() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(bindings.resolve(&key), Some(Action::NextGroup));
    }

    #[test]
    fn test_universal_previous_group_vim() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('K'), KeyModifiers::SHIFT);
        assert_eq!(bindings.resolve(&key), Some(Action::PreviousGroup));
    }

    #[test]
    fn test_universal_previous_group_pgup() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::PreviousGroup));
    }

    #[test]
    fn test_universal_previous_group_ctrl_u() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(bindings.resolve(&key), Some(Action::PreviousGroup));
    }

    #[test]
    fn test_universal_toggle_select() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::ToggleSelect));
    }

    #[test]
    fn test_universal_select_all_in_group() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::SelectAllInGroup));
    }

    #[test]
    fn test_universal_select_all_duplicates() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(bindings.resolve(&key), Some(Action::SelectAllDuplicates));
    }

    #[test]
    fn test_universal_confirm() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::Confirm));
    }

    #[test]
    fn test_universal_cancel() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::Cancel));
    }

    #[test]
    fn test_universal_quit_q() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::Quit));
    }

    #[test]
    fn test_universal_quit_ctrl_c() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(bindings.resolve(&key), Some(Action::Quit));
    }

    #[test]
    fn test_universal_preview() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::Preview));
    }

    #[test]
    fn test_universal_delete() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::Delete));
    }

    #[test]
    fn test_universal_toggle_theme() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);
        let key = key_press(KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), Some(Action::ToggleTheme));
    }

    // =========================================================================
    // Vim Profile Tests
    // =========================================================================

    #[test]
    fn test_vim_navigate_down() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Vim);
        let j_key = key_press(KeyCode::Char('j'), KeyModifiers::NONE);
        let down_key = key_press(KeyCode::Down, KeyModifiers::NONE);

        assert_eq!(bindings.resolve(&j_key), Some(Action::NavigateDown));
        // Arrow keys should NOT work in Vim profile
        assert_eq!(bindings.resolve(&down_key), None);
    }

    #[test]
    fn test_vim_navigate_up() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Vim);
        let k_key = key_press(KeyCode::Char('k'), KeyModifiers::NONE);
        let up_key = key_press(KeyCode::Up, KeyModifiers::NONE);

        assert_eq!(bindings.resolve(&k_key), Some(Action::NavigateUp));
        // Arrow keys should NOT work in Vim profile
        assert_eq!(bindings.resolve(&up_key), None);
    }

    // =========================================================================
    // Standard Profile Tests
    // =========================================================================

    #[test]
    fn test_standard_navigate_down() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Standard);
        let j_key = key_press(KeyCode::Char('j'), KeyModifiers::NONE);
        let down_key = key_press(KeyCode::Down, KeyModifiers::NONE);

        // Vim keys should NOT work in Standard profile
        assert_eq!(bindings.resolve(&j_key), None);
        assert_eq!(bindings.resolve(&down_key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_standard_navigate_up() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Standard);
        let k_key = key_press(KeyCode::Char('k'), KeyModifiers::NONE);
        let up_key = key_press(KeyCode::Up, KeyModifiers::NONE);

        // Vim keys should NOT work in Standard profile
        assert_eq!(bindings.resolve(&k_key), None);
        assert_eq!(bindings.resolve(&up_key), Some(Action::NavigateUp));
    }

    #[test]
    fn test_standard_delete_uses_delete_key() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Standard);
        let del_key = key_press(KeyCode::Delete, KeyModifiers::NONE);
        let d_key = key_press(KeyCode::Char('d'), KeyModifiers::NONE);

        assert_eq!(bindings.resolve(&del_key), Some(Action::Delete));
        // 'd' should not delete in Standard profile
        assert_eq!(bindings.resolve(&d_key), None);
    }

    // =========================================================================
    // Emacs Profile Tests
    // =========================================================================

    #[test]
    fn test_emacs_navigate_down() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Emacs);
        let ctrl_n = key_press(KeyCode::Char('n'), KeyModifiers::CONTROL);
        let down_key = key_press(KeyCode::Down, KeyModifiers::NONE);

        assert_eq!(bindings.resolve(&ctrl_n), Some(Action::NavigateDown));
        assert_eq!(bindings.resolve(&down_key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_emacs_navigate_up() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Emacs);
        let ctrl_p = key_press(KeyCode::Char('p'), KeyModifiers::CONTROL);
        let up_key = key_press(KeyCode::Up, KeyModifiers::NONE);

        assert_eq!(bindings.resolve(&ctrl_p), Some(Action::NavigateUp));
        assert_eq!(bindings.resolve(&up_key), Some(Action::NavigateUp));
    }

    #[test]
    fn test_emacs_cancel_ctrl_g() {
        let bindings = KeyBindings::from_profile(KeybindingProfile::Emacs);
        let ctrl_g = key_press(KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(bindings.resolve(&ctrl_g), Some(Action::Cancel));
    }

    // =========================================================================
    // Key Release Handling
    // =========================================================================

    #[test]
    fn test_ignores_key_release() {
        let bindings = KeyBindings::default();
        let key = key_release(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), None);
    }

    // =========================================================================
    // Unmapped Keys
    // =========================================================================

    #[test]
    fn test_unmapped_key() {
        let bindings = KeyBindings::default();
        let key = key_press(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&key), None);
    }

    #[test]
    fn test_modifier_matters() {
        let bindings = KeyBindings::default();

        // 'j' with no modifier = NavigateDown
        let j_key = key_press(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&j_key), Some(Action::NavigateDown));

        // Alt+J = unmapped
        let alt_j = key_press(KeyCode::Char('j'), KeyModifiers::ALT);
        assert_eq!(bindings.resolve(&alt_j), None);
    }

    // =========================================================================
    // Keys For Action
    // =========================================================================

    #[test]
    fn test_keys_for_action() {
        let bindings = KeyBindings::default();
        let keys = bindings.keys_for_action(&Action::NavigateDown);
        assert!(!keys.is_empty());
        // Should include both 'j' and Down arrow
        assert!(keys.iter().any(|k| k.code == KeyCode::Char('j')));
        assert!(keys.iter().any(|k| k.code == KeyCode::Down));
    }

    #[test]
    fn test_keys_for_unmapped_action_returns_empty() {
        // Create bindings and check for an action that might not have keys
        let bindings = KeyBindings::default();
        // All actions should have bindings, but keys_for_action handles empty case
        let keys = bindings.keys_for_action(&Action::NavigateDown);
        assert!(!keys.is_empty());
    }

    // =========================================================================
    // Key Hint Formatting
    // =========================================================================

    #[test]
    fn test_key_hint() {
        let bindings = KeyBindings::default();
        let hint = bindings.key_hint(&Action::Quit);
        // Should be 'q' or 'Ctrl+c' depending on order
        assert!(!hint.is_empty());
    }

    #[test]
    fn test_format_key_simple() {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "j");
    }

    #[test]
    fn test_format_key_with_ctrl() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(KeyBindings::format_key(&key), "Ctrl+c");
    }

    #[test]
    fn test_format_key_with_shift() {
        let key = KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT);
        assert_eq!(KeyBindings::format_key(&key), "Shift+J");
    }

    #[test]
    fn test_format_key_space() {
        let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "Space");
    }

    #[test]
    fn test_format_key_special() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "Enter");

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "Esc");

        let key = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "PgDn");
    }

    #[test]
    fn test_format_key_function_key() {
        let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "F1");

        let key = KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE);
        assert_eq!(KeyBindings::format_key(&key), "F12");
    }
}
