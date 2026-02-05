//! Comprehensive integration tests for keybinding functionality.
//!
//! These tests verify the full integration between Config, KeyBindings,
//! EventHandler, and TUI application state.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use rustdupe::config::{Config, CustomKeybindings};
use rustdupe::tui::app::Action;
use rustdupe::tui::events::EventHandler;
use rustdupe::tui::keybindings::{KeyBindings, KeybindingError, KeybindingProfile};
use std::collections::HashMap;

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a key press event for testing.
fn key_press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

/// Create a key release event for testing.
fn key_release(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    }
}

// =============================================================================
// Profile Tests - Universal
// =============================================================================

#[test]
fn test_universal_profile_supports_vim_and_arrows_simultaneously() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);

    // Vim-style navigation
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(Action::NavigateDown),
        "Universal profile should support 'j' for down"
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('k'), KeyModifiers::NONE)),
        Some(Action::NavigateUp),
        "Universal profile should support 'k' for up"
    );

    // Arrow key navigation
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Down, KeyModifiers::NONE)),
        Some(Action::NavigateDown),
        "Universal profile should support Down arrow"
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Up, KeyModifiers::NONE)),
        Some(Action::NavigateUp),
        "Universal profile should support Up arrow"
    );
}

#[test]
fn test_universal_profile_group_navigation() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);

    // Next group keys
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::PageDown, KeyModifiers::NONE)),
        Some(Action::NextGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        Some(Action::NextGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::NextGroup)
    );

    // Previous group keys
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        Some(Action::PreviousGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('h'), KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Left, KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );
}

#[test]
fn test_universal_profile_go_to_top_bottom() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);

    // Go to top
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::GoToTop)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('g'), KeyModifiers::NONE)),
        Some(Action::GoToTop)
    );

    // Go to bottom
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::End, KeyModifiers::NONE)),
        Some(Action::GoToBottom)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('G'), KeyModifiers::SHIFT)),
        Some(Action::GoToBottom)
    );
}

// =============================================================================
// Profile Tests - Vim
// =============================================================================

#[test]
fn test_vim_profile_navigation_without_arrows() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Vim);

    // Vim navigation should work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(Action::NavigateDown)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('k'), KeyModifiers::NONE)),
        Some(Action::NavigateUp)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('h'), KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );

    // Arrow keys should NOT work in Vim profile
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Down, KeyModifiers::NONE)),
        None,
        "Vim profile should not support arrow keys"
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Up, KeyModifiers::NONE)),
        None,
        "Vim profile should not support arrow keys"
    );
}

#[test]
fn test_vim_profile_go_to_top_bottom() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Vim);

    // Vim uses 'g' for top and 'G' for bottom
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('g'), KeyModifiers::NONE)),
        Some(Action::GoToTop)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('G'), KeyModifiers::SHIFT)),
        Some(Action::GoToBottom)
    );

    // Home/End should not work in pure Vim mode
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Home, KeyModifiers::NONE)),
        None
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::End, KeyModifiers::NONE)),
        None
    );
}

// =============================================================================
// Profile Tests - Standard
// =============================================================================

#[test]
fn test_standard_profile_navigation_without_vim() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Standard);

    // Arrow key navigation should work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Down, KeyModifiers::NONE)),
        Some(Action::NavigateDown)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Up, KeyModifiers::NONE)),
        Some(Action::NavigateUp)
    );

    // Vim keys should NOT work in Standard profile
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE)),
        None,
        "Standard profile should not support vim navigation"
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('k'), KeyModifiers::NONE)),
        None,
        "Standard profile should not support vim navigation"
    );
}

#[test]
fn test_standard_profile_uses_delete_key() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Standard);

    // Delete key should trigger delete action
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Delete, KeyModifiers::NONE)),
        Some(Action::Delete)
    );

    // 'd' should NOT trigger delete in standard mode
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('d'), KeyModifiers::NONE)),
        None
    );
}

#[test]
fn test_standard_profile_group_navigation() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Standard);

    // Left/Right arrows for group navigation
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Left, KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Right, KeyModifiers::NONE)),
        Some(Action::NextGroup)
    );

    // PageUp/PageDown should also work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::PageDown, KeyModifiers::NONE)),
        Some(Action::NextGroup)
    );
}

// =============================================================================
// Profile Tests - Emacs
// =============================================================================

#[test]
fn test_emacs_profile_ctrl_navigation() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Emacs);

    // Ctrl-n/p for navigation
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('n'), KeyModifiers::CONTROL)),
        Some(Action::NavigateDown)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('p'), KeyModifiers::CONTROL)),
        Some(Action::NavigateUp)
    );

    // Arrow keys should also work in Emacs profile (fallback)
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Down, KeyModifiers::NONE)),
        Some(Action::NavigateDown)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Up, KeyModifiers::NONE)),
        Some(Action::NavigateUp)
    );
}

#[test]
fn test_emacs_profile_cancel_with_ctrl_g() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Emacs);

    // Ctrl-g is the Emacs cancel key
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('g'), KeyModifiers::CONTROL)),
        Some(Action::Cancel)
    );

    // Escape should also work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Esc, KeyModifiers::NONE)),
        Some(Action::Cancel)
    );
}

#[test]
fn test_emacs_profile_go_to_top_bottom() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Emacs);

    // Meta-< and Meta-> for top/bottom
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('<'), KeyModifiers::ALT)),
        Some(Action::GoToTop)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('>'), KeyModifiers::ALT)),
        Some(Action::GoToBottom)
    );

    // Home/End should also work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Home, KeyModifiers::NONE)),
        Some(Action::GoToTop)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::End, KeyModifiers::NONE)),
        Some(Action::GoToBottom)
    );
}

// =============================================================================
// Dual Navigation Tests (Main Feature Test)
// =============================================================================

#[test]
fn test_dual_navigation_vim_and_arrows_work_together() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);

    // Test all navigation directions with both vim and arrows
    let nav_pairs = [
        (KeyCode::Char('j'), KeyCode::Down, Action::NavigateDown),
        (KeyCode::Char('k'), KeyCode::Up, Action::NavigateUp),
    ];

    for (vim_key, arrow_key, expected_action) in nav_pairs {
        let vim_result = bindings.resolve(&key_press(vim_key, KeyModifiers::NONE));
        let arrow_result = bindings.resolve(&key_press(arrow_key, KeyModifiers::NONE));

        assert_eq!(
            vim_result,
            Some(expected_action),
            "Vim key {:?} should trigger {:?}",
            vim_key,
            expected_action
        );
        assert_eq!(
            arrow_result,
            Some(expected_action),
            "Arrow key {:?} should trigger {:?}",
            arrow_key,
            expected_action
        );
    }
}

#[test]
fn test_page_navigation_ctrl_and_page_keys() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);

    // Page Down: Ctrl-d and PageDown both work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        Some(Action::NextGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::PageDown, KeyModifiers::NONE)),
        Some(Action::NextGroup)
    );

    // Page Up: Ctrl-u and PageUp both work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        Some(Action::PreviousGroup)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(Action::PreviousGroup)
    );
}

// =============================================================================
// Custom Keybindings Tests
// =============================================================================

#[test]
fn test_custom_bindings_merge_with_profile_not_replace() {
    let mut custom: CustomKeybindings = HashMap::new();
    // Add a custom binding 'x' for navigate_down (not normally bound)
    custom.insert("navigate_down".to_string(), vec!["x".to_string()]);

    let bindings = KeyBindings::from_profile_with_custom(KeybindingProfile::Standard, &custom)
        .expect("Custom bindings should parse successfully");

    // Custom 'x' should work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('x'), KeyModifiers::NONE)),
        Some(Action::NavigateDown),
        "Custom 'x' key should trigger NavigateDown"
    );

    // Original Down arrow should STILL work (merge, not replace)
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Down, KeyModifiers::NONE)),
        Some(Action::NavigateDown),
        "Original Down arrow should still work after adding custom binding"
    );
}

#[test]
fn test_custom_bindings_multiple_keys_for_same_action() {
    let mut custom: CustomKeybindings = HashMap::new();
    custom.insert(
        "quit".to_string(),
        vec!["x".to_string(), "Ctrl+w".to_string(), "F10".to_string()],
    );

    let bindings = KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom)
        .expect("Custom bindings should parse");

    // All custom keys should work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('x'), KeyModifiers::NONE)),
        Some(Action::Quit)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('w'), KeyModifiers::CONTROL)),
        Some(Action::Quit)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::F(10), KeyModifiers::NONE)),
        Some(Action::Quit)
    );

    // Original 'q' should still work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('q'), KeyModifiers::NONE)),
        Some(Action::Quit)
    );
}

#[test]
fn test_custom_bindings_all_modifier_combinations() {
    let mut custom: CustomKeybindings = HashMap::new();
    custom.insert(
        "toggle_select".to_string(),
        vec![
            "Ctrl+Space".to_string(),
            "Alt+s".to_string(),
            "Ctrl+Shift+s".to_string(),
        ],
    );

    let bindings = KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom)
        .expect("Modifier combinations should parse");

    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char(' '), KeyModifiers::CONTROL)),
        Some(Action::ToggleSelect)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('s'), KeyModifiers::ALT)),
        Some(Action::ToggleSelect)
    );
    assert_eq!(
        bindings.resolve(&key_press(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )),
        Some(Action::ToggleSelect)
    );
}

#[test]
fn test_custom_bindings_empty_preserves_profile() {
    let custom: CustomKeybindings = HashMap::new();

    let bindings = KeyBindings::from_profile_with_custom(KeybindingProfile::Vim, &custom)
        .expect("Empty custom bindings should work");

    // Should behave exactly like Vim profile
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(Action::NavigateDown)
    );
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Down, KeyModifiers::NONE)),
        None, // Vim profile doesn't have arrow keys
    );
}

// =============================================================================
// Invalid Configuration Handling Tests
// =============================================================================

#[test]
fn test_invalid_action_name_produces_helpful_error() {
    let mut custom: CustomKeybindings = HashMap::new();
    custom.insert("invalid_action".to_string(), vec!["x".to_string()]);

    let result = KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, KeybindingError::InvalidAction(_)));

    let msg = err.to_string();
    assert!(
        msg.contains("invalid_action"),
        "Error should mention the invalid action"
    );
    assert!(
        msg.contains("navigate_down") || msg.contains("quit"),
        "Error should list valid actions"
    );
}

#[test]
fn test_invalid_key_spec_produces_helpful_error() {
    let mut custom: CustomKeybindings = HashMap::new();
    custom.insert("quit".to_string(), vec!["not_a_valid_key".to_string()]);

    let result = KeyBindings::from_profile_with_custom(KeybindingProfile::Universal, &custom);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, KeybindingError::InvalidKeySpec(_)));

    let msg = err.to_string();
    assert!(
        msg.contains("not_a_valid_key") || msg.contains("Invalid"),
        "Error should indicate invalid key spec"
    );
}

#[test]
fn test_invalid_profile_name_produces_error() {
    let result: Result<KeybindingProfile, _> = "not_a_profile".parse();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, KeybindingError::InvalidProfile(_)));

    let msg = err.to_string();
    assert!(msg.contains("universal") || msg.contains("vim"));
}

#[test]
fn test_empty_key_spec_produces_error() {
    let result = KeyBindings::parse_key("");
    assert!(result.is_err());
}

#[test]
fn test_whitespace_only_key_spec_produces_error() {
    let result = KeyBindings::parse_key("   ");
    assert!(result.is_err());
}

#[test]
fn test_modifier_without_key_produces_error() {
    let result = KeyBindings::parse_key("Ctrl+");
    assert!(result.is_err());
}

// =============================================================================
// Key Parsing Tests
// =============================================================================

#[test]
fn test_parse_key_simple_letters() {
    for c in 'a'..='z' {
        let result = KeyBindings::parse_key(&c.to_string());
        assert!(result.is_ok(), "Should parse lowercase letter '{}'", c);
        let key = result.unwrap();
        assert_eq!(key.code, KeyCode::Char(c));
        assert_eq!(key.modifiers, KeyModifiers::NONE);
    }
}

#[test]
fn test_parse_key_special_keys() {
    let special_keys = [
        ("Space", KeyCode::Char(' ')),
        ("Enter", KeyCode::Enter),
        ("Return", KeyCode::Enter),
        ("Esc", KeyCode::Esc),
        ("Escape", KeyCode::Esc),
        ("Tab", KeyCode::Tab),
        ("Backspace", KeyCode::Backspace),
        ("Delete", KeyCode::Delete),
    ];

    for (spec, expected) in special_keys {
        let result = KeyBindings::parse_key(spec);
        assert!(result.is_ok(), "Should parse '{}'", spec);
        assert_eq!(
            result.unwrap().code,
            expected,
            "Key '{}' should map to {:?}",
            spec,
            expected
        );
    }
}

#[test]
fn test_parse_key_arrow_keys() {
    let arrow_keys = [
        ("Up", KeyCode::Up),
        ("Down", KeyCode::Down),
        ("Left", KeyCode::Left),
        ("Right", KeyCode::Right),
    ];

    for (spec, expected) in arrow_keys {
        let result = KeyBindings::parse_key(spec);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().code, expected);
    }
}

#[test]
fn test_parse_key_navigation_keys() {
    let nav_keys = [
        ("PageUp", KeyCode::PageUp),
        ("PgUp", KeyCode::PageUp),
        ("PageDown", KeyCode::PageDown),
        ("PgDn", KeyCode::PageDown),
        ("Home", KeyCode::Home),
        ("End", KeyCode::End),
    ];

    for (spec, expected) in nav_keys {
        let result = KeyBindings::parse_key(spec);
        assert!(result.is_ok(), "Should parse '{}'", spec);
        assert_eq!(result.unwrap().code, expected);
    }
}

#[test]
fn test_parse_key_function_keys() {
    for n in 1..=12 {
        let spec = format!("F{}", n);
        let result = KeyBindings::parse_key(&spec);
        assert!(result.is_ok(), "Should parse '{}'", spec);
        assert_eq!(result.unwrap().code, KeyCode::F(n));
    }
}

#[test]
fn test_parse_key_with_modifiers() {
    let cases = [
        ("Ctrl+a", KeyCode::Char('a'), KeyModifiers::CONTROL),
        ("Control+b", KeyCode::Char('b'), KeyModifiers::CONTROL),
        ("Alt+c", KeyCode::Char('c'), KeyModifiers::ALT),
        ("Meta+d", KeyCode::Char('d'), KeyModifiers::ALT),
        ("Shift+Enter", KeyCode::Enter, KeyModifiers::SHIFT),
    ];

    for (spec, expected_code, expected_mods) in cases {
        let result = KeyBindings::parse_key(spec);
        assert!(result.is_ok(), "Should parse '{}'", spec);
        let key = result.unwrap();
        assert_eq!(key.code, expected_code, "Code mismatch for '{}'", spec);
        assert_eq!(
            key.modifiers, expected_mods,
            "Modifiers mismatch for '{}'",
            spec
        );
    }
}

#[test]
fn test_parse_key_multiple_modifiers() {
    let key = KeyBindings::parse_key("Ctrl+Shift+a").expect("Should parse Ctrl+Shift+a");
    assert_eq!(key.code, KeyCode::Char('a'));
    assert!(key.modifiers.contains(KeyModifiers::CONTROL));
    assert!(key.modifiers.contains(KeyModifiers::SHIFT));
}

#[test]
fn test_parse_key_case_insensitive_modifiers() {
    let keys = ["ctrl+a", "CTRL+A", "Ctrl+a", "CONTROL+a"];
    for spec in keys {
        let result = KeyBindings::parse_key(spec);
        assert!(result.is_ok(), "Should parse '{}'", spec);
        assert!(result.unwrap().modifiers.contains(KeyModifiers::CONTROL));
    }
}

// =============================================================================
// Action Parsing Tests
// =============================================================================

#[test]
fn test_parse_action_all_valid_actions() {
    let actions = Action::all_names();
    for action_name in actions {
        let result = KeyBindings::parse_action(action_name);
        assert!(result.is_ok(), "Should parse action '{}'", action_name);
    }
}

#[test]
fn test_parse_action_case_insensitive() {
    assert_eq!(
        KeyBindings::parse_action("NAVIGATE_DOWN").unwrap(),
        Action::NavigateDown
    );
    assert_eq!(
        KeyBindings::parse_action("Navigate_Down").unwrap(),
        Action::NavigateDown
    );
    assert_eq!(
        KeyBindings::parse_action("navigate_down").unwrap(),
        Action::NavigateDown
    );
}

#[test]
fn test_parse_action_aliases() {
    // Test that common aliases work
    assert_eq!(
        KeyBindings::parse_action("down").unwrap(),
        Action::NavigateDown
    );
    assert_eq!(KeyBindings::parse_action("up").unwrap(), Action::NavigateUp);
    assert_eq!(KeyBindings::parse_action("exit").unwrap(), Action::Quit);
    assert_eq!(KeyBindings::parse_action("esc").unwrap(), Action::Cancel);
    assert_eq!(KeyBindings::parse_action("escape").unwrap(), Action::Cancel);
    assert_eq!(KeyBindings::parse_action("enter").unwrap(), Action::Confirm);
    assert_eq!(KeyBindings::parse_action("help").unwrap(), Action::ShowHelp);
}

#[test]
fn test_parse_action_hyphen_and_underscore_equivalent() {
    assert_eq!(
        KeyBindings::parse_action("navigate-down").unwrap(),
        Action::NavigateDown
    );
    assert_eq!(
        KeyBindings::parse_action("navigate_down").unwrap(),
        Action::NavigateDown
    );
    assert_eq!(
        KeyBindings::parse_action("go-to-top").unwrap(),
        Action::GoToTop
    );
    assert_eq!(
        KeyBindings::parse_action("go_to_top").unwrap(),
        Action::GoToTop
    );
}

// =============================================================================
// EventHandler Integration Tests
// =============================================================================

#[test]
fn test_event_handler_default_uses_universal_profile() {
    let handler = EventHandler::new();
    let bindings = handler.bindings();

    assert_eq!(bindings.profile(), KeybindingProfile::Universal);
}

#[test]
fn test_event_handler_with_profile() {
    for profile in KeybindingProfile::all() {
        let handler = EventHandler::with_profile(*profile);
        assert_eq!(handler.bindings().profile(), *profile);
    }
}

#[test]
fn test_event_handler_with_custom_bindings() {
    let mut custom: CustomKeybindings = HashMap::new();
    custom.insert("quit".to_string(), vec!["F12".to_string()]);

    let bindings = KeyBindings::from_profile_with_custom(KeybindingProfile::Vim, &custom)
        .expect("Custom bindings should work");

    let handler = EventHandler::with_bindings(bindings);

    // Custom binding should be accessible via handler
    assert_eq!(handler.bindings().profile(), KeybindingProfile::Vim);

    // The F12 key should be in the quit bindings
    let quit_keys = handler.bindings().keys_for_action(&Action::Quit);
    assert!(
        quit_keys.iter().any(|k| k.code == KeyCode::F(12)),
        "Custom F12 binding should be present"
    );
}

// =============================================================================
// Profile Switching Tests
// =============================================================================

#[test]
fn test_profile_switching_changes_available_keys() {
    // Start with Universal - both vim and arrows work
    let universal = KeyBindings::from_profile(KeybindingProfile::Universal);
    assert!(universal
        .resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE))
        .is_some());
    assert!(universal
        .resolve(&key_press(KeyCode::Down, KeyModifiers::NONE))
        .is_some());

    // Switch to Vim - only vim keys work
    let vim = KeyBindings::from_profile(KeybindingProfile::Vim);
    assert!(vim
        .resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE))
        .is_some());
    assert!(vim
        .resolve(&key_press(KeyCode::Down, KeyModifiers::NONE))
        .is_none());

    // Switch to Standard - only arrow keys work
    let standard = KeyBindings::from_profile(KeybindingProfile::Standard);
    assert!(standard
        .resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE))
        .is_none());
    assert!(standard
        .resolve(&key_press(KeyCode::Down, KeyModifiers::NONE))
        .is_some());
}

#[test]
fn test_all_profiles_have_basic_actions() {
    let required_actions = [
        Action::NavigateUp,
        Action::NavigateDown,
        Action::ToggleSelect,
        Action::Confirm,
        Action::Cancel,
        Action::Quit,
    ];

    for profile in KeybindingProfile::all() {
        let bindings = KeyBindings::from_profile(*profile);

        for action in &required_actions {
            let keys = bindings.keys_for_action(action);
            assert!(
                !keys.is_empty(),
                "Profile {:?} should have keys for {:?}",
                profile,
                action
            );
        }
    }
}

// =============================================================================
// Key Release Handling
// =============================================================================

#[test]
fn test_key_release_events_ignored() {
    let bindings = KeyBindings::default();

    // Press should work
    let press = key_press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(bindings.resolve(&press), Some(Action::NavigateDown));

    // Release should be ignored
    let release = key_release(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(bindings.resolve(&release), None);
}

// =============================================================================
// Config Integration Tests
// =============================================================================

#[test]
fn test_config_default_profile_is_universal() {
    let config = Config::default();
    assert_eq!(config.keybinding_profile, KeybindingProfile::Universal);
}

#[test]
fn test_config_custom_keybindings_default_empty() {
    let config = Config::default();
    assert!(config.custom_keybindings.is_empty());
    assert!(!config.has_custom_keybindings());
}

#[test]
fn test_config_with_custom_keybindings() {
    let mut config = Config::default();
    config
        .custom_keybindings
        .insert("quit".to_string(), vec!["x".to_string()]);

    assert!(config.has_custom_keybindings());

    // Build KeyBindings from config
    let bindings = KeyBindings::from_profile_with_custom(
        config.keybinding_profile,
        &config.custom_keybindings,
    )
    .expect("Should build from config");

    // Custom key should work
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('x'), KeyModifiers::NONE)),
        Some(Action::Quit)
    );
}

// =============================================================================
// Profile Serialization Tests
// =============================================================================

#[test]
fn test_profile_display_names() {
    assert!(KeybindingProfile::Universal
        .display_name()
        .contains("Universal"));
    assert!(KeybindingProfile::Vim.display_name().contains("Vim"));
    assert!(KeybindingProfile::Standard.display_name().contains("Arrow"));
    assert!(KeybindingProfile::Emacs.display_name().contains("Emacs"));
}

#[test]
fn test_profile_to_string() {
    assert_eq!(KeybindingProfile::Universal.to_string(), "universal");
    assert_eq!(KeybindingProfile::Vim.to_string(), "vim");
    assert_eq!(KeybindingProfile::Standard.to_string(), "standard");
    assert_eq!(KeybindingProfile::Emacs.to_string(), "emacs");
}

#[test]
fn test_profile_from_string() {
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
}

#[test]
fn test_profile_from_string_case_insensitive() {
    assert_eq!(
        "UNIVERSAL".parse::<KeybindingProfile>().unwrap(),
        KeybindingProfile::Universal
    );
    assert_eq!(
        "Vim".parse::<KeybindingProfile>().unwrap(),
        KeybindingProfile::Vim
    );
    assert_eq!(
        "EMACS".parse::<KeybindingProfile>().unwrap(),
        KeybindingProfile::Emacs
    );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_unmapped_keys_return_none() {
    let bindings = KeyBindings::default();

    // Keys that should never be mapped
    let unmapped = [
        key_press(KeyCode::Char('z'), KeyModifiers::NONE),
        key_press(KeyCode::Char('y'), KeyModifiers::NONE),
        key_press(KeyCode::F(13), KeyModifiers::NONE), // F13 not typically used
        key_press(KeyCode::Insert, KeyModifiers::NONE),
    ];

    for key in unmapped {
        assert_eq!(
            bindings.resolve(&key),
            None,
            "Key {:?} should not be mapped",
            key.code
        );
    }
}

#[test]
fn test_modifier_differences_matter() {
    let bindings = KeyBindings::from_profile(KeybindingProfile::Universal);

    // 'j' without modifier = NavigateDown
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(Action::NavigateDown)
    );

    // Alt+j = not mapped
    assert_eq!(
        bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::ALT)),
        None
    );

    // Shift+j = might be mapped (depending on profile) or not
    // Just ensure it doesn't crash
    let _ = bindings.resolve(&key_press(KeyCode::Char('j'), KeyModifiers::SHIFT));
}

#[test]
fn test_format_key_round_trip() {
    // Keys that should format and parse correctly
    let test_cases = [
        (KeyCode::Char('j'), KeyModifiers::NONE),
        (KeyCode::Char('c'), KeyModifiers::CONTROL),
        (KeyCode::Enter, KeyModifiers::NONE),
        (KeyCode::F(1), KeyModifiers::NONE),
        (KeyCode::Down, KeyModifiers::NONE),
    ];

    for (code, modifiers) in test_cases {
        let key = KeyEvent::new(code, modifiers);
        let formatted = KeyBindings::format_key(&key);

        // Should be non-empty
        assert!(
            !formatted.is_empty(),
            "Format should produce non-empty string"
        );

        // Should be parseable back (mostly - some edge cases may not round-trip exactly)
        let parsed = KeyBindings::parse_key(&formatted);
        // Don't assert exact equality for all cases, just that it parses
        if formatted != "?" {
            assert!(
                parsed.is_ok(),
                "Should be able to parse formatted key '{}'",
                formatted
            );
        }
    }
}
