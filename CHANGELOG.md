# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-02-06

### Added
- **Multi-Directory Scanning**: Support for multiple root paths in a single scan with intelligent path overlap detection and canonicalization.
- **Directory Groups**: Named directory sets (`--group NAME=PATH`) for easier organization and batch selection in the TUI.
- **Perceptual Image Hashing**: Detect visually similar images using pHash, dHash, and aHash algorithms with high-performance BK-tree similarity clustering.
- **Fuzzy Text Matching**: Detect similar text documents (PDF, DOCX, TXT) using 64-bit SimHash fingerprinting and word 3-grams.
- **Two-Stage Bloom Filters**: Drastic performance improvement using probabilistic filters for file size and 4KB prehashes to minimize expensive full-file hashing.
- **Layered Configuration System**: Support for `config.toml` (XDG-compliant) with hierarchical overrides (defaults < config file < named profiles < environment variables < CLI flags).
- **Platform-Adaptive Keybindings**: Predefined profiles for Vim, Standard, and Emacs, with a "Universal" default that supports both Vim keys and arrow keys.
- **Accessible Mode**: New `--accessible` flag for screen reader compatibility, featuring ASCII visuals, reduced refresh rates, and simplified progress bars.
- **Real-time Metrics & ETA**: Exponential Moving Average (EMA) based ETA calculation and stable throughput tracking (files/sec and MB/sec).
- **TUI Search & Filter**: Real-time filtering of duplicate groups using literal substring matching or case-insensitive regular expressions.
- **TUI Bulk Selection**: Advanced selection by directory, extension, or relative size/date with a safety-first confirmation workflow and undo (Shift+U) support.
- **TUI Expand/Collapse**: Hierarchical duplicate group view ('e' key) to efficiently browse large scan results.
- **Sortable Columns**: Toggle sorting by size, path, date, or duplicate count in the TUI using Tab and Shift+Tab.
- **Memory-Mapped Hashing**: Performance boost for large files using `--mmap` with BLAKE3's parallel memory-mapped hashing.
- **Adaptive I/O Buffering**: Automatic adjustment of read buffers (64KB to 16MB) based on file size and available system memory.
- **Enhanced Export**: HTML reports now support optional image thumbnails (base64 or linked), and all formats support a new `--export-selected` option.
- **Structured Error Codes**: Machine-readable exit codes (RD000-RD130) and JSON-formatted error reporting for automation.

### Changed
- The `scan` command now accepts multiple positional path arguments.
- Default configuration format migrated to TOML for better readability and maintainability.
- Enhanced final scan summary with colored tables, per-phase duration breakdown, and Bloom filter efficiency metrics.
- Improved error messages across the application with rich context and actionable suggestions.

### Fixed
- Improved TUI layout stability on small terminal windows.
- Resolved Unicode normalization edge cases for macOS (NFD) and Linux/Windows (NFC) path comparisons.
- Enhanced signal handling for graceful shutdown during multi-threaded operations.

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

[Unreleased]: https://github.com/MasuRii/RustDupe/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/MasuRii/RustDupe/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MasuRii/RustDupe/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MasuRii/RustDupe/releases/tag/v0.1.0
