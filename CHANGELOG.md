# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/rustdupe/rustdupe/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rustdupe/rustdupe/releases/tag/v0.1.0
