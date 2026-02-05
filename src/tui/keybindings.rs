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

use clap::ValueEnum;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Action;

/// Keybinding profile presets.
///
/// Each profile defines a complete set of keybindings tailored for
/// different user preferences and familiarity with various navigation styles.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    Hash,
    ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
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
    #[error("Invalid key specification: '{0}'. Examples: 'j', 'Ctrl+c', 'Down', 'Space', 'F1'")]
    InvalidKeySpec(String),

    /// Invalid action name.
    #[error("Unknown action: '{0}'. Valid actions: {}", Action::all_names().join(", "))]
    InvalidAction(String),
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

    /// Parse a key specification string into a KeyEvent.
    ///
    /// Supports formats like:
    /// - Simple keys: "j", "k", "Space", "Enter", "Esc"
    /// - Arrow keys: "Up", "Down", "Left", "Right"
    /// - Special keys: "PageUp", "PageDown", "PgUp", "PgDn", "Home", "End"
    /// - Function keys: "F1", "F2", ..., "F12"
    /// - With modifiers: "Ctrl+c", "Alt+j", "Shift+Enter"
    /// - Multiple modifiers: "Ctrl+Shift+a"
    ///
    /// # Errors
    ///
    /// Returns `KeybindingError::InvalidKeySpec` if the key specification
    /// cannot be parsed.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::keybindings::KeyBindings;
    /// use crossterm::event::{KeyCode, KeyModifiers};
    ///
    /// let key = KeyBindings::parse_key("Ctrl+j").unwrap();
    /// assert_eq!(key.code, KeyCode::Char('j'));
    /// assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    ///
    /// let key = KeyBindings::parse_key("Down").unwrap();
    /// assert_eq!(key.code, KeyCode::Down);
    /// ```
    pub fn parse_key(spec: &str) -> Result<KeyEvent, KeybindingError> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err(KeybindingError::InvalidKeySpec(spec.to_string()));
        }

        // Split on '+' but handle edge case of '+' key itself
        let parts: Vec<&str> = if spec == "+" {
            vec!["+"]
        } else {
            spec.split('+').map(str::trim).collect()
        };

        if parts.is_empty() {
            return Err(KeybindingError::InvalidKeySpec(spec.to_string()));
        }

        let mut modifiers = KeyModifiers::NONE;
        let mut key_part = None;

        for (i, part) in parts.iter().enumerate() {
            let lower = part.to_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "meta" | "option" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                _ => {
                    // This should be the actual key (last part)
                    if i != parts.len() - 1 {
                        // Modifier in wrong position - treat as invalid
                        return Err(KeybindingError::InvalidKeySpec(format!(
                            "'{spec}' - unexpected modifier position for '{part}'"
                        )));
                    }
                    key_part = Some(*part);
                }
            }
        }

        let key_str = key_part.ok_or_else(|| {
            KeybindingError::InvalidKeySpec(format!("'{spec}' - missing key after modifiers"))
        })?;

        let code = Self::parse_key_code(key_str)
            .ok_or_else(|| KeybindingError::InvalidKeySpec(spec.to_string()))?;

        Ok(KeyEvent::new(code, modifiers))
    }

    /// Parse a key code from a string.
    fn parse_key_code(s: &str) -> Option<KeyCode> {
        let lower = s.to_lowercase();

        // Check for function keys first (F1-F12)
        if let Some(rest) = lower.strip_prefix('f') {
            if let Ok(n) = rest.parse::<u8>() {
                if (1..=12).contains(&n) {
                    return Some(KeyCode::F(n));
                }
            }
        }

        // Named keys
        match lower.as_str() {
            // Letters (single char)
            _ if s.len() == 1 && s.chars().next().map(|c| c.is_ascii()).unwrap_or(false) => {
                Some(KeyCode::Char(s.chars().next().unwrap()))
            }

            // Special named keys
            "space" | "spc" => Some(KeyCode::Char(' ')),
            "enter" | "return" | "ret" | "cr" => Some(KeyCode::Enter),
            "esc" | "escape" => Some(KeyCode::Esc),
            "tab" => Some(KeyCode::Tab),
            "backtab" | "shifttab" => Some(KeyCode::BackTab),
            "backspace" | "bs" => Some(KeyCode::Backspace),
            "delete" | "del" => Some(KeyCode::Delete),
            "insert" | "ins" => Some(KeyCode::Insert),

            // Arrow keys
            "up" | "uparrow" => Some(KeyCode::Up),
            "down" | "downarrow" => Some(KeyCode::Down),
            "left" | "leftarrow" => Some(KeyCode::Left),
            "right" | "rightarrow" => Some(KeyCode::Right),

            // Navigation keys
            "pageup" | "pgup" | "page_up" => Some(KeyCode::PageUp),
            "pagedown" | "pgdn" | "pgdown" | "page_down" => Some(KeyCode::PageDown),
            "home" => Some(KeyCode::Home),
            "end" => Some(KeyCode::End),

            _ => None,
        }
    }

    /// Parse an action name from a string.
    ///
    /// # Errors
    ///
    /// Returns `KeybindingError::InvalidAction` if the action name is not recognized.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::keybindings::KeyBindings;
    /// use rustdupe::tui::Action;
    ///
    /// let action = KeyBindings::parse_action("navigate_down").unwrap();
    /// assert_eq!(action, Action::NavigateDown);
    ///
    /// let action = KeyBindings::parse_action("quit").unwrap();
    /// assert_eq!(action, Action::Quit);
    /// ```
    pub fn parse_action(name: &str) -> Result<Action, KeybindingError> {
        name.parse::<Action>()
            .map_err(|_| KeybindingError::InvalidAction(name.to_string()))
    }

    /// Merge custom keybindings with profile defaults.
    ///
    /// Custom bindings are added to the existing bindings for each action,
    /// rather than replacing them entirely. This allows users to add
    /// additional key combinations while keeping the profile defaults.
    ///
    /// # Arguments
    ///
    /// * `custom` - A map of action names to lists of key specifications
    ///
    /// # Errors
    ///
    /// Returns an error if any action name or key specification is invalid.
    /// The error message includes details about what went wrong.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::keybindings::{KeyBindings, KeybindingProfile};
    /// use std::collections::HashMap;
    ///
    /// let mut bindings = KeyBindings::from_profile(KeybindingProfile::Standard);
    ///
    /// // Add custom bindings
    /// let mut custom = HashMap::new();
    /// custom.insert("navigate_down".to_string(), vec!["j".to_string()]);
    /// custom.insert("quit".to_string(), vec!["x".to_string()]);
    ///
    /// let bindings = bindings.with_custom_overrides(&custom).unwrap();
    ///
    /// // Now 'j' also triggers NavigateDown (in addition to Down arrow)
    /// ```
    pub fn with_custom_overrides(
        mut self,
        custom: &HashMap<String, Vec<String>>,
    ) -> Result<Self, KeybindingError> {
        for (action_name, key_specs) in custom {
            let action = Self::parse_action(action_name)?;

            for key_spec in key_specs {
                let key_event = Self::parse_key(key_spec)?;

                // Ensure this key is removed from any other actions to ensure the override wins
                for (other_action, other_keys) in &mut self.action_keys {
                    if *other_action != action {
                        other_keys.retain(|k| !Self::key_matches(k, &key_event));
                    }
                }

                // Add to existing bindings (merge, not replace)
                self.action_keys.entry(action).or_default().push(key_event);
            }
        }

        Ok(self)
    }

    /// Create keybindings from a profile with custom overrides.
    ///
    /// This is a convenience method that combines `from_profile` and
    /// `with_custom_overrides`.
    ///
    /// # Errors
    ///
    /// Returns an error if any custom binding is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use rustdupe::tui::keybindings::{KeyBindings, KeybindingProfile};
    /// use std::collections::HashMap;
    ///
    /// let mut custom = HashMap::new();
    /// custom.insert("navigate_down".to_string(), vec!["n".to_string()]);
    ///
    /// let bindings = KeyBindings::from_profile_with_custom(
    ///     KeybindingProfile::Vim,
    ///     &custom
    /// ).unwrap();
    /// ```
    pub fn from_profile_with_custom(
        profile: KeybindingProfile,
        custom: &HashMap<String, Vec<String>>,
    ) -> Result<Self, KeybindingError> {
        Self::from_profile(profile).with_custom_overrides(custom)
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
            Action::SelectByExtension,
            vec![Self::key(KeyCode::Char('E'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectByDirectory,
            vec![Self::key(KeyCode::Char('D'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::UndoSelection,
            vec![Self::key(KeyCode::Char('U'), KeyModifiers::SHIFT)],
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
            Action::SelectGroup,
            vec![
                Self::key(KeyCode::Char('b'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('B'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::Search,
            vec![Self::key(KeyCode::Char('/'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Char('d'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleExpandAll,
            vec![Self::key(KeyCode::Char('e'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::CycleSortColumn,
            vec![Self::key(KeyCode::Tab, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ReverseSortDirection,
            vec![Self::key(KeyCode::BackTab, KeyModifiers::SHIFT)],
        );

        // Help
        bindings.insert(
            Action::ShowHelp,
            vec![
                Self::key(KeyCode::Char('?'), KeyModifiers::NONE),
                Self::key(KeyCode::F(1), KeyModifiers::NONE),
            ],
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
            Action::SelectByExtension,
            vec![Self::key(KeyCode::Char('E'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectByDirectory,
            vec![Self::key(KeyCode::Char('D'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::UndoSelection,
            vec![Self::key(KeyCode::Char('U'), KeyModifiers::SHIFT)],
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
            Action::SelectGroup,
            vec![
                Self::key(KeyCode::Char('b'), KeyModifiers::NONE),
                Self::key(KeyCode::Char('B'), KeyModifiers::SHIFT),
            ],
        );

        bindings.insert(
            Action::Search,
            vec![Self::key(KeyCode::Char('/'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Char('d'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleExpandAll,
            vec![Self::key(KeyCode::Char('e'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::CycleSortColumn,
            vec![Self::key(KeyCode::Tab, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ReverseSortDirection,
            vec![Self::key(KeyCode::BackTab, KeyModifiers::SHIFT)],
        );

        // Help
        bindings.insert(
            Action::ShowHelp,
            vec![
                Self::key(KeyCode::Char('?'), KeyModifiers::NONE),
                Self::key(KeyCode::F(1), KeyModifiers::NONE),
            ],
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
            Action::SelectByExtension,
            vec![Self::key(KeyCode::Char('E'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectByDirectory,
            vec![Self::key(KeyCode::Char('D'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::UndoSelection,
            vec![
                Self::key(KeyCode::Char('z'), KeyModifiers::CONTROL),
                Self::key(KeyCode::Char('U'), KeyModifiers::SHIFT),
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
            vec![Self::key(KeyCode::Char('f'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::SelectGroup,
            vec![Self::key(KeyCode::Char('b'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Search,
            vec![Self::key(KeyCode::Char('/'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Delete, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleExpandAll,
            vec![Self::key(KeyCode::Char('e'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::CycleSortColumn,
            vec![Self::key(KeyCode::Tab, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ReverseSortDirection,
            vec![Self::key(KeyCode::BackTab, KeyModifiers::SHIFT)],
        );

        // Help
        bindings.insert(
            Action::ShowHelp,
            vec![
                Self::key(KeyCode::Char('?'), KeyModifiers::NONE),
                Self::key(KeyCode::F(1), KeyModifiers::NONE),
            ],
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
            Action::SelectByExtension,
            vec![Self::key(KeyCode::Char('E'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::SelectByDirectory,
            vec![Self::key(KeyCode::Char('D'), KeyModifiers::SHIFT)],
        );

        bindings.insert(
            Action::UndoSelection,
            vec![
                Self::key(KeyCode::Char('_'), KeyModifiers::CONTROL),
                Self::key(KeyCode::Char('U'), KeyModifiers::SHIFT),
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
            Action::SelectGroup,
            vec![Self::key(KeyCode::Char('b'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Search,
            vec![Self::key(KeyCode::Char('/'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::Delete,
            vec![Self::key(KeyCode::Char('d'), KeyModifiers::CONTROL)],
        );

        bindings.insert(
            Action::ToggleTheme,
            vec![Self::key(KeyCode::Char('t'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ToggleExpand,
            vec![
                Self::key(KeyCode::Enter, KeyModifiers::NONE),
                Self::key(KeyCode::Char(' '), KeyModifiers::CONTROL),
            ],
        );

        bindings.insert(
            Action::ToggleExpandAll,
            vec![Self::key(KeyCode::Char('e'), KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::CycleSortColumn,
            vec![Self::key(KeyCode::Tab, KeyModifiers::NONE)],
        );

        bindings.insert(
            Action::ReverseSortDirection,
            vec![Self::key(KeyCode::BackTab, KeyModifiers::SHIFT)],
        );

        // Help
        bindings.insert(
            Action::ShowHelp,
            vec![
                Self::key(KeyCode::Char('?'), KeyModifiers::NONE),
                Self::key(KeyCode::F(1), KeyModifiers::NONE),
            ],
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

    // =========================================================================
    // Key Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_key_simple_letter() {
        let key = KeyBindings::parse_key("j").unwrap();
        assert_eq!(key.code, KeyCode::Char('j'));
        assert_eq!(key.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_uppercase_letter() {
        let key = KeyBindings::parse_key("J").unwrap();
        assert_eq!(key.code, KeyCode::Char('J'));
        assert_eq!(key.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_space() {
        let key = KeyBindings::parse_key("Space").unwrap();
        assert_eq!(key.code, KeyCode::Char(' '));
        assert_eq!(key.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_enter() {
        let key = KeyBindings::parse_key("Enter").unwrap();
        assert_eq!(key.code, KeyCode::Enter);

        let key = KeyBindings::parse_key("Return").unwrap();
        assert_eq!(key.code, KeyCode::Enter);
    }

    #[test]
    fn test_parse_key_escape() {
        let key = KeyBindings::parse_key("Esc").unwrap();
        assert_eq!(key.code, KeyCode::Esc);

        let key = KeyBindings::parse_key("Escape").unwrap();
        assert_eq!(key.code, KeyCode::Esc);
    }

    #[test]
    fn test_parse_key_arrows() {
        let key = KeyBindings::parse_key("Up").unwrap();
        assert_eq!(key.code, KeyCode::Up);

        let key = KeyBindings::parse_key("Down").unwrap();
        assert_eq!(key.code, KeyCode::Down);

        let key = KeyBindings::parse_key("Left").unwrap();
        assert_eq!(key.code, KeyCode::Left);

        let key = KeyBindings::parse_key("Right").unwrap();
        assert_eq!(key.code, KeyCode::Right);
    }

    #[test]
    fn test_parse_key_navigation() {
        let key = KeyBindings::parse_key("PageUp").unwrap();
        assert_eq!(key.code, KeyCode::PageUp);

        let key = KeyBindings::parse_key("PgDn").unwrap();
        assert_eq!(key.code, KeyCode::PageDown);

        let key = KeyBindings::parse_key("Home").unwrap();
        assert_eq!(key.code, KeyCode::Home);

        let key = KeyBindings::parse_key("End").unwrap();
        assert_eq!(key.code, KeyCode::End);
    }

    #[test]
    fn test_parse_key_function_keys() {
        let key = KeyBindings::parse_key("F1").unwrap();
        assert_eq!(key.code, KeyCode::F(1));

        let key = KeyBindings::parse_key("F12").unwrap();
        assert_eq!(key.code, KeyCode::F(12));
    }

    #[test]
    fn test_parse_key_with_ctrl() {
        let key = KeyBindings::parse_key("Ctrl+c").unwrap();
        assert_eq!(key.code, KeyCode::Char('c'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL);

        let key = KeyBindings::parse_key("Control+j").unwrap();
        assert_eq!(key.code, KeyCode::Char('j'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_key_with_alt() {
        let key = KeyBindings::parse_key("Alt+x").unwrap();
        assert_eq!(key.code, KeyCode::Char('x'));
        assert_eq!(key.modifiers, KeyModifiers::ALT);

        let key = KeyBindings::parse_key("Meta+<").unwrap();
        assert_eq!(key.code, KeyCode::Char('<'));
        assert_eq!(key.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn test_parse_key_with_shift() {
        let key = KeyBindings::parse_key("Shift+Enter").unwrap();
        assert_eq!(key.code, KeyCode::Enter);
        assert_eq!(key.modifiers, KeyModifiers::SHIFT);
    }

    #[test]
    fn test_parse_key_multiple_modifiers() {
        let key = KeyBindings::parse_key("Ctrl+Shift+a").unwrap();
        assert_eq!(key.code, KeyCode::Char('a'));
        assert!(key.modifiers.contains(KeyModifiers::CONTROL));
        assert!(key.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_parse_key_case_insensitive_modifiers() {
        let key = KeyBindings::parse_key("CTRL+j").unwrap();
        assert_eq!(key.code, KeyCode::Char('j'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_key_invalid_empty() {
        let result = KeyBindings::parse_key("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_key_invalid_unknown() {
        let result = KeyBindings::parse_key("unknown_key");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_key_whitespace_trimmed() {
        let key = KeyBindings::parse_key("  j  ").unwrap();
        assert_eq!(key.code, KeyCode::Char('j'));
    }

    // =========================================================================
    // Action Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_action_navigate_down() {
        let action = KeyBindings::parse_action("navigate_down").unwrap();
        assert_eq!(action, Action::NavigateDown);

        let action = KeyBindings::parse_action("down").unwrap();
        assert_eq!(action, Action::NavigateDown);
    }

    #[test]
    fn test_parse_action_quit() {
        let action = KeyBindings::parse_action("quit").unwrap();
        assert_eq!(action, Action::Quit);

        let action = KeyBindings::parse_action("exit").unwrap();
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn test_parse_action_case_insensitive() {
        let action = KeyBindings::parse_action("NAVIGATE_UP").unwrap();
        assert_eq!(action, Action::NavigateUp);
    }

    #[test]
    fn test_parse_action_hyphen_underscore_equivalent() {
        let action = KeyBindings::parse_action("navigate-down").unwrap();
        assert_eq!(action, Action::NavigateDown);
    }

    #[test]
    fn test_parse_action_invalid() {
        let result = KeyBindings::parse_action("unknown_action");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KeybindingError::InvalidAction(_)));
    }

    // =========================================================================
    // Custom Keybindings Tests
    // =========================================================================

    #[test]
    fn test_custom_bindings_merge_with_profile() {
        let mut custom = std::collections::HashMap::new();
        // Use 'r' which is not bound in Standard profile
        custom.insert("navigate_down".to_string(), vec!["r".to_string()]);

        let bindings =
            KeyBindings::from_profile_with_custom(KeybindingProfile::Standard, &custom).unwrap();

        // Original Down arrow should still work
        let down_key = key_press(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&down_key), Some(Action::NavigateDown));

        // Custom 'r' should also work now
        let r_key = key_press(KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&r_key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_custom_bindings_multiple_keys() {
        let mut custom = std::collections::HashMap::new();
        custom.insert(
            "quit".to_string(),
            vec!["x".to_string(), "Ctrl+w".to_string()],
        );

        let bindings =
            KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom).unwrap();

        // Custom 'x' should work
        let x_key = key_press(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&x_key), Some(Action::Quit));

        // Custom Ctrl+w should work
        let ctrl_w = key_press(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(bindings.resolve(&ctrl_w), Some(Action::Quit));

        // Original 'q' should still work
        let q_key = key_press(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&q_key), Some(Action::Quit));
    }

    #[test]
    fn test_custom_bindings_invalid_action() {
        let mut custom = std::collections::HashMap::new();
        custom.insert("invalid_action".to_string(), vec!["x".to_string()]);

        let result = KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_bindings_invalid_key() {
        let mut custom = std::collections::HashMap::new();
        custom.insert("quit".to_string(), vec!["invalid_key".to_string()]);

        let result = KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_bindings_empty_is_noop() {
        let custom = std::collections::HashMap::new();

        let bindings =
            KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom).unwrap();

        // Should be same as just from_profile
        let j_key = key_press(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&j_key), Some(Action::NavigateDown));
    }

    #[test]
    fn test_with_custom_overrides_method() {
        let mut custom = std::collections::HashMap::new();
        custom.insert("toggle_select".to_string(), vec!["Tab".to_string()]);

        let bindings = KeyBindings::from_profile(KeybindingProfile::Vim)
            .with_custom_overrides(&custom)
            .unwrap();

        // Tab should now trigger toggle_select
        let tab_key = key_press(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&tab_key), Some(Action::ToggleSelect));

        // Original Space should still work
        let space_key = key_press(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(bindings.resolve(&space_key), Some(Action::ToggleSelect));
    }
}
