//! Unicode path normalization utilities.
//!
//! This module provides functions for normalizing Unicode paths to NFC form,
//! which is critical for consistent path comparison across platforms.
//!
//! # Background
//!
//! macOS uses NFD (Decomposed) normalization for file paths, while Windows
//! and Linux typically use NFC (Composed) normalization. This means the same
//! visual filename can have different byte representations:
//!
//! - NFC: `café.txt` - 'é' is U+00E9 (single code point)
//! - NFD: `café.txt` - 'e' U+0065 + combining acute accent U+0301
//!
//! Without normalization, these would compare as different paths.
//!
//! # Example
//!
//! ```
//! use rustdupe::scanner::path_utils::{normalize_path_str, paths_equal};
//!
//! // These look identical but have different Unicode representations
//! let nfc = "café.txt";       // é is U+00E9
//! let nfd = "cafe\u{0301}.txt";  // e + combining accent
//!
//! // After normalization, they match
//! assert_eq!(normalize_path_str(nfc), normalize_path_str(nfd));
//! assert!(paths_equal(nfc, nfd));
//! ```

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Normalize a path string to NFC (Composed) form.
///
/// This function converts any Unicode string to NFC normalization form,
/// ensuring consistent byte representation for path comparison.
///
/// # Arguments
///
/// * `s` - The string to normalize
///
/// # Returns
///
/// A new String in NFC form. If the input is already NFC, the characters
/// are unchanged (though a new String is still allocated).
///
/// # Example
///
/// ```
/// use rustdupe::scanner::path_utils::normalize_path_str;
///
/// let nfd = "cafe\u{0301}.txt"; // NFD form
/// let normalized = normalize_path_str(nfd);
/// assert_eq!(normalized, "café.txt"); // NFC form
/// ```
#[must_use]
pub fn normalize_path_str(s: &str) -> String {
    s.nfc().collect()
}

/// Normalize a path string to NFC, returning a Cow for efficiency.
///
/// This is more efficient than [`normalize_path_str`] when the input
/// is already in NFC form, as it avoids allocation in that case.
///
/// # Arguments
///
/// * `s` - The string to normalize
///
/// # Returns
///
/// A `Cow<str>` that is either borrowed (if already NFC) or owned (if
/// normalization was needed).
///
/// # Example
///
/// ```
/// use rustdupe::scanner::path_utils::normalize_path_str_cow;
///
/// let already_nfc = "café.txt"; // Already NFC
/// let result = normalize_path_str_cow(already_nfc);
/// // May avoid allocation if input is already normalized
/// ```
#[must_use]
pub fn normalize_path_str_cow(s: &str) -> Cow<'_, str> {
    // Check if already NFC by comparing with normalized form
    let normalized: String = s.nfc().collect();
    if normalized == s {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(normalized)
    }
}

/// Normalize a [`PathBuf`] to NFC form.
///
/// Converts the path to a string, normalizes to NFC, and returns a new PathBuf.
/// If the path contains invalid UTF-8, returns the original path unchanged.
///
/// # Arguments
///
/// * `path` - The path to normalize
///
/// # Returns
///
/// A new PathBuf with NFC-normalized components.
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use rustdupe::scanner::path_utils::normalize_pathbuf;
///
/// let path = PathBuf::from("documents/cafe\u{0301}.txt");
/// let normalized = normalize_pathbuf(&path);
/// assert_eq!(normalized, PathBuf::from("documents/café.txt"));
/// ```
#[must_use]
pub fn normalize_pathbuf(path: &Path) -> PathBuf {
    // Try to convert to str; if it fails (invalid UTF-8), return original
    match path.to_str() {
        Some(s) => PathBuf::from(normalize_path_str(s)),
        None => path.to_path_buf(),
    }
}

/// Check if two path strings are equal after NFC normalization.
///
/// This is useful for comparing paths that may have different Unicode
/// normalization forms but represent the same logical file.
///
/// # Arguments
///
/// * `a` - First path string
/// * `b` - Second path string
///
/// # Returns
///
/// `true` if both paths normalize to the same NFC string.
///
/// # Example
///
/// ```
/// use rustdupe::scanner::path_utils::paths_equal;
///
/// let nfc = "café.txt";
/// let nfd = "cafe\u{0301}.txt";
/// assert!(paths_equal(nfc, nfd));
///
/// let different = "coffee.txt";
/// assert!(!paths_equal(nfc, different));
/// ```
#[must_use]
pub fn paths_equal(a: &str, b: &str) -> bool {
    normalize_path_str(a) == normalize_path_str(b)
}

/// Check if two [`Path`]s are equal after NFC normalization.
///
/// # Arguments
///
/// * `a` - First path
/// * `b` - Second path
///
/// # Returns
///
/// `true` if both paths normalize to the same NFC form.
/// Returns `false` if either path contains invalid UTF-8.
///
/// # Example
///
/// ```
/// use std::path::Path;
/// use rustdupe::scanner::path_utils::paths_equal_normalized;
///
/// let a = Path::new("café.txt");
/// let b = Path::new("cafe\u{0301}.txt");
/// assert!(paths_equal_normalized(a, b));
/// ```
#[must_use]
pub fn paths_equal_normalized(a: &Path, b: &Path) -> bool {
    match (a.to_str(), b.to_str()) {
        (Some(a_str), Some(b_str)) => paths_equal(a_str, b_str),
        _ => false,
    }
}

/// Check if a string is already in NFC form.
///
/// # Arguments
///
/// * `s` - The string to check
///
/// # Returns
///
/// `true` if the string is already in NFC form (no normalization needed).
///
/// # Example
///
/// ```
/// use rustdupe::scanner::path_utils::is_nfc;
///
/// assert!(is_nfc("café.txt"));      // NFC form
/// assert!(!is_nfc("cafe\u{0301}.txt")); // NFD form
/// ```
#[must_use]
pub fn is_nfc(s: &str) -> bool {
    unicode_normalization::is_nfc(s)
}

/// Create a normalized comparison key for a path.
///
/// This is useful for using paths as HashMap/HashSet keys where
/// Unicode normalization differences should not matter.
///
/// # Arguments
///
/// * `path` - The path to create a key for
///
/// # Returns
///
/// A String suitable for use as a comparison/hash key.
/// If the path contains invalid UTF-8, returns the lossy conversion.
///
/// # Example
///
/// ```
/// use std::path::Path;
/// use std::collections::HashSet;
/// use rustdupe::scanner::path_utils::path_key;
///
/// let mut seen = HashSet::new();
/// seen.insert(path_key(Path::new("café.txt")));
///
/// // NFD version of same path already in set
/// let nfd_path = Path::new("cafe\u{0301}.txt");
/// assert!(seen.contains(&path_key(nfd_path)));
/// ```
#[must_use]
pub fn path_key(path: &Path) -> String {
    normalize_path_str(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_str_nfc_unchanged() {
        // Already NFC - should remain the same
        let nfc = "café.txt";
        assert_eq!(normalize_path_str(nfc), nfc);
    }

    #[test]
    fn test_normalize_path_str_nfd_to_nfc() {
        // NFD input should become NFC
        let nfd = "cafe\u{0301}.txt"; // e + combining acute accent
        let expected = "café.txt"; // é as single code point
        assert_eq!(normalize_path_str(nfd), expected);
    }

    #[test]
    fn test_normalize_path_str_ascii_unchanged() {
        // Pure ASCII is always NFC
        let ascii = "hello.txt";
        assert_eq!(normalize_path_str(ascii), ascii);
    }

    #[test]
    fn test_normalize_path_str_empty() {
        // Empty string handling
        assert_eq!(normalize_path_str(""), "");
    }

    #[test]
    fn test_normalize_path_str_mixed_content() {
        // Mixed ASCII and Unicode
        let mixed = "path/to/file_cafe\u{0301}_2024.txt";
        let expected = "path/to/file_café_2024.txt";
        assert_eq!(normalize_path_str(mixed), expected);
    }

    #[test]
    fn test_normalize_path_str_multiple_accents() {
        // Multiple combining characters
        let nfd = "re\u{0301}sume\u{0301}.txt"; // résumé in NFD
        let nfc = "résumé.txt";
        assert_eq!(normalize_path_str(nfd), nfc);
    }

    #[test]
    fn test_normalize_pathbuf() {
        let path = PathBuf::from("docs/cafe\u{0301}.txt");
        let normalized = normalize_pathbuf(&path);
        assert_eq!(normalized, PathBuf::from("docs/café.txt"));
    }

    #[test]
    fn test_normalize_pathbuf_already_nfc() {
        let path = PathBuf::from("docs/café.txt");
        let normalized = normalize_pathbuf(&path);
        assert_eq!(normalized, PathBuf::from("docs/café.txt"));
    }

    #[test]
    fn test_paths_equal_nfc_vs_nfd() {
        let nfc = "café.txt";
        let nfd = "cafe\u{0301}.txt";
        assert!(paths_equal(nfc, nfd));
    }

    #[test]
    fn test_paths_equal_different() {
        assert!(!paths_equal("café.txt", "coffee.txt"));
    }

    #[test]
    fn test_paths_equal_normalized() {
        let a = Path::new("café.txt");
        let b = Path::new("cafe\u{0301}.txt");
        assert!(paths_equal_normalized(a, b));

        let c = Path::new("other.txt");
        assert!(!paths_equal_normalized(a, c));
    }

    #[test]
    fn test_is_nfc() {
        assert!(is_nfc("café.txt")); // NFC
        assert!(!is_nfc("cafe\u{0301}.txt")); // NFD
        assert!(is_nfc("hello.txt")); // ASCII is NFC
        assert!(is_nfc("")); // Empty is NFC
    }

    #[test]
    fn test_path_key() {
        let nfc_path = Path::new("café.txt");
        let nfd_path = Path::new("cafe\u{0301}.txt");

        assert_eq!(path_key(nfc_path), path_key(nfd_path));
    }

    #[test]
    fn test_normalize_path_str_cow_already_nfc() {
        let nfc = "café.txt";
        let result = normalize_path_str_cow(nfc);
        // Should be borrowed since already NFC
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_normalize_path_str_cow_needs_conversion() {
        let nfd = "cafe\u{0301}.txt";
        let result = normalize_path_str_cow(nfd);
        // Should be owned since conversion needed
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "café.txt");
    }

    #[test]
    fn test_combining_characters() {
        // Test various combining characters
        // ñ can be: U+00F1 (NFC) or n + U+0303 (NFD)
        let nfc = "español.txt";
        let nfd = "espan\u{0303}ol.txt";
        assert!(paths_equal(nfc, nfd));
    }

    #[test]
    fn test_hangul_normalization() {
        // Korean Hangul can have multiple normalization forms
        // 가 can be: U+AC00 (NFC) or ㄱ + ㅏ (U+1100 + U+1161, NFD)
        let nfc = "가.txt";
        let nfd = "\u{1100}\u{1161}.txt";
        assert!(paths_equal(nfc, nfd));
    }
}
