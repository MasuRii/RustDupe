# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-02-05

### Added
- Persistent Hash Caching: SQLite-backed cache for skipping unchanged files during rescans.
- Session Management: Save/load duplicate reviews with SHA256 integrity verification.
- Reference Directories: Protected path support to prevent deletion of original source files.
- Advanced Filtering: New flags for modification date ranges, regex patterns, and file type categories (images, videos, etc.).
- HTML Report Export: Self-contained, responsive HTML reports with dark mode support.
- Shell Script Generation: Safety-first POSIX and PowerShell scripts for reviewing and executing deletions.
- TUI Batch Selection: New operations to select all duplicates, oldest/newest files, and folder-based selection.
- Dry-run Mode: `--dry-run` and `--analyze-only` flags for safe, read-only analysis.
- TUI Theming: Support for Light, Dark, and Auto themes with runtime switching ('t' key).
- Improved CLI Help: Grouped options into logical categories and added descriptive examples.
- Comprehensive module and item documentation for all new features.
- Additional integration tests for sessions, caching, and cross-platform path edge cases.

### Changed
- Refactored `DuplicateGroup` to store `FileEntry` objects for metadata-aware batch operations.
- Updated session format to version 2 to support enhanced data structures.
- Improved path handling to be more robust against special characters and long paths.

## [0.1.0] - 2026-02-01

### Added
- Multi-phase duplicate detection engine (Size grouping → Prehash → Full hash).
- Parallel directory walking using `jwalk`.
- Fast content hashing using BLAKE3 algorithm.
- Interactive TUI for reviewing duplicate groups and managing deletions.
- File preview functionality (text, binary hex dump, image metadata).
- Safe deletion with system trash support via `trash` crate.
- Hardlink detection and skipping to prevent false duplicate identification.
- Unicode path normalization for cross-platform compatibility (NFC/NFD).
- JSON and CSV output formats for automation and scripting.
- Support for `.gitignore` files and custom ignore patterns.
- Global verbosity and quiet mode flags.
- Comprehensive test suite including unit, integration, and property-based tests.
- Graceful shutdown handling for long-running operations.

### Changed
- Initial project release.

[Unreleased]: https://github.com/MasuRii/RustDupe/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/MasuRii/RustDupe/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MasuRii/RustDupe/releases/tag/v0.1.0
