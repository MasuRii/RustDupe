//! TUI theming support.
//!
//! This module provides the `Theme` struct which defines the color palette
//! for the TUI. It supports light and dark themes, as well as automatic
//! detection based on terminal environment.

use ratatui::style::Color;

/// A collection of colors used for TUI components.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub danger: Color,
    pub success: Color,
    pub reference: Color,
    pub dim: Color,
    pub normal: Color,
    pub inverted_fg: Color,
}

impl Theme {
    /// Create a high-contrast dark theme (default).
    ///
    /// Palette:
    /// - Primary: Cyan (headers, borders)
    /// - Secondary: Yellow (selections, highlights)
    /// - Danger: Red (deletions, errors)
    /// - Success: Green (saved space, originals)
    /// - Reference: Blue (protected files)
    /// - Dim: DarkGray (secondary text)
    /// - Normal: White (main text)
    /// - Inverted FG: Black (text on colored background)
    pub fn dark() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Yellow,
            danger: Color::Red,
            success: Color::Green,
            reference: Color::Blue,
            dim: Color::DarkGray,
            normal: Color::White,
            inverted_fg: Color::Black,
        }
    }

    /// Create a high-contrast light theme.
    ///
    /// Palette:
    /// - Primary: Blue (headers, borders)
    /// - Secondary: Magenta (selections, highlights)
    /// - Danger: Red (deletions, errors)
    /// - Success: Green (saved space, originals)
    /// - Reference: Cyan (protected files)
    /// - Dim: Gray (secondary text)
    /// - Normal: Black (main text)
    /// - Inverted FG: White (text on colored background)
    pub fn light() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Magenta,
            danger: Color::Red,
            success: Color::Green,
            reference: Color::Cyan,
            dim: Color::Gray,
            normal: Color::Black,
            inverted_fg: Color::White,
        }
    }

    /// Detect terminal theme or return dark theme as default.
    pub fn auto() -> Self {
        if is_light_terminal() {
            Self::light()
        } else {
            Self::dark()
        }
    }

    /// Check if this is a light theme.
    pub fn is_light(&self) -> bool {
        self.normal == Color::Black
    }
}

/// Simple heuristic to detect if the terminal is light-themed.
///
/// Checks common environment variables used by some terminal emulators.
fn is_light_terminal() -> bool {
    // COLORFGBG is set by some terminals (e.g. rxvt, xterm, konsole).
    // Format is "fg;bg", where bg is typically 0-15 or a color index.
    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = colorfgbg.split(';').collect();
        if let Some(bg) = parts.last() {
            if let Ok(bg_num) = bg.parse::<u32>() {
                // Heuristic: lower numbers are usually dark
                // 0=black, 7=gray, 15=white
                return bg_num >= 7 && bg_num != 8; // 8 is usually dark gray
            }
        }
    }

    // Check for TERM_PROGRAM and specific light themes if possible
    // (e.g. Apple_Terminal often defaults to light unless configured otherwise)

    false // Default to dark if unsure
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
