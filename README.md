# RustDupe

[![CI](https://github.com/MasuRii/RustDupe/actions/workflows/ci.yml/badge.svg)](https://github.com/MasuRii/RustDupe/actions/workflows/ci.yml)
[![Crates.io Version](https://img.shields.io/crates/v/rustdupe.svg?style=flat-square&color=orange)](https://crates.io/crates/rustdupe)
[![Crates.io Downloads](https://img.shields.io/crates/d/rustdupe.svg?style=flat-square&color=blue)](https://crates.io/crates/rustdupe)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.85%2B-blue.svg?style=flat-square)](https://www.rust-lang.org)

**Smart Duplicate File Finder** — A high-performance, cross-platform duplicate file finder built in Rust with an interactive TUI.

![ScreenShot](public/images/rustdupe_tuiscreenshot.png)

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [CLI Reference](#cli-reference)
- [Performance](#performance)
- [Contributing](#contributing)
- [License](#license)

## Features

- **High Performance**: Parallel directory walking and BLAKE3 hashing for maximum speed.
- **Bloom Filters**: Two-stage probabilistic filtering for sub-millisecond duplicate rejection.
- **Hash Caching**: Persistent SQLite cache for lightning-fast rescans by skipping unchanged files.
- **Perceptual Hashing**: Detect visually similar images (pHash, dHash, aHash) with Hamming distance matching.
- **Fuzzy Text Matching**: Detect similar documents (PDF, DOCX, TXT) using SimHash fingerprinting.
- **Interactive TUI**: Review groups with real-time search, bulk selection, sorting, and expand/collapse support.
- **Accessible Mode**: Screen reader friendly interface with ASCII visuals and optimized refresh rates.
- **Keybinding Profiles**: Switch between Universal, Vim, Standard, and Emacs profiles.
- **Session Management**: Save and resume duplicate review sessions with checksum-verified integrity.
- **Reference Directories**: Protect original source directories from accidental deletion.
- **Advanced Export**: Generate self-contained HTML reports with image previews and safety-first shell scripts.
- **Multi-Phase Optimization**:
  1. Group by file size (instant filtering).
  2. Bloom filter Stage 1 (size-based rejection).
  3. Compare 4KB pre-hashes (fast rejection).
  4. Bloom filter Stage 2 (prehash-based rejection).
  5. Full content hash for final confirmation.
  6. Optional byte-by-byte verification (paranoid mode).
- **Safe Deletion**: Moves files to system trash by default (cross-platform support).
- **Hardlink Aware**: Automatically detects and skips hardlinks (same inode) to prevent false positives.
- **Unicode Support**: Handles macOS NFD vs. Windows/Linux NFC normalization issues.
- **Theming**: Light, Dark, and Auto-detected terminal themes.

## Installation

### From crates.io (Recommended)

```bash
cargo install rustdupe
```

> **Requires Rust 1.85 or later.** Install Rust via [rustup](https://rustup.rs/).

### Pre-built Binaries

Download the latest release for your platform from the [GitHub Releases](https://github.com/MasuRii/RustDupe/releases) page.

| Platform | Architecture | Download |
|----------|--------------|----------|
| Linux | x86_64 | `rustdupe-*-x86_64-unknown-linux-gnu` |
| Linux (musl) | x86_64 | `rustdupe-*-x86_64-unknown-linux-musl` |
| macOS | x86_64 | `rustdupe-*-x86_64-apple-darwin` |
| macOS | Apple Silicon | `rustdupe-*-aarch64-apple-darwin` |
| Windows | x86_64 | `rustdupe-*-x86_64-pc-windows-msvc.exe` |

### From Source

```bash
git clone https://github.com/MasuRii/RustDupe.git
cd rustdupe
cargo build --release
```

The binary will be available at `target/release/rustdupe`.

## Usage

### Basic Scan (Interactive TUI)

```bash
# Scan a single directory
rustdupe scan ~/Downloads

# Scan multiple directories together
rustdupe scan /path/to/photos /external/backup/photos

# Use named groups for easier selection
rustdupe scan --group personal=~/Photos --group work=~/Work/Photos
```

### Incremental Scanning (Cache)

Speed up subsequent scans by enabling the persistent hash cache. Configuration can also be stored in a file.

```bash
# Uses default platform-specific cache path
rustdupe scan ~/Documents

# Use a named profile from your config file
rustdupe scan . --profile fast-scan
```

### Similarity Detection

Find images and documents that are visually or structurally similar, not just bitwise identical.

```bash
# Find similar images (resized, re-encoded, etc.)
rustdupe scan ~/Photos --similar-images

# Find similar documents (PDF, DOCX, TXT)
rustdupe scan ~/Documents --similar-documents

# Adjust similarity threshold (Hamming distance)
rustdupe scan ~/Photos --similar-images --similarity-threshold 15
```

### Workflow Persistence (Sessions)

Save your progress and resume your duplicate review later.

```bash
# Save scan results to a session file
rustdupe scan ~/Photos --save-session backup.json

# Load and resume a session in the TUI
rustdupe load backup.json

# Load a session and export to a different format
rustdupe load backup.json --output html --output-file report.html
```

### Protected Paths (Reference Directories)

Protect "golden" copies of your files. Files in reference directories are never selected by batch operations and cannot be manually selected for deletion.

```bash
rustdupe scan ./working-dir --reference ./backup-drive/originals
```

### Advanced Export (Reports & Scripts)

Generate reports and scripts for automated or manual review.

```bash
# Generate HTML report with embedded image thumbnails
rustdupe scan ~/Photos --similar-images --output html --html-thumbnails > report.html

# Export only the files you selected in the TUI
rustdupe load session.json --export-selected --output script > cleanup.sh
```

### Accessibility & Compatibility

Screen reader support and platform-specific keybindings.

```bash
# Enable accessible mode (ASCII borders, no animations)
rustdupe scan . --accessible

# Use Vim-style keybindings (hjkl)
rustdupe scan . --keys vim
```

## Configuration

RustDupe supports a `config.toml` file for persistent settings and named profiles.

- **Linux/macOS**: `~/.config/rustdupe/config.toml`
- **Windows**: `%APPDATA%\rustdupe\config.toml`

### Example Configuration

```toml
theme = "dark"
keybinding_profile = "universal"

[accessibility]
enabled = false

[profile.photos]
similar_images = true
similarity_threshold = 10
file_type = ["images"]

[keybindings.custom]
Search = ["/"]
Export = ["x"]
```

## CLI Reference

```text
Usage: rustdupe [OPTIONS] <COMMAND>

Commands:
  scan  Scan directories for duplicate files
  load  Load a previously saved session
  help  Print this message

Global Options:
  -v, --verbose...           Increase verbosity
      --profile <NAME>       Load a named configuration profile
      --keybinding-profile   TUI profile (universal, vim, standard, emacs)
      --accessible           Enable screen reader compatible mode
      --json-errors          Output errors as JSON

Scan Options:
  [PATH]...                  One or more directories to scan
  -o, --output <FORMAT>      tui, json, csv, html, session, script
      --group <NAME=PATH>    Named directory groups
      --similar-images       Enable perceptual image hashing
      --similar-documents    Enable fuzzy text matching
      --mmap                 Enable memory-mapped hashing
      --strict               Fail-fast on any error
      --export-selected      Export only selected files

Filtering Options:
      --min-size <SIZE>      Min size (e.g., 1MB, 1GB)
      --file-type <TYPE>     images, videos, audio, documents, archives
      --regex <PATTERN>      Include files matching regex
  -i, --ignore <PATTERN>     Glob patterns to ignore

Safety Options:
      --dry-run              Read-only mode (no deletions)
      --reference <PATH>     Protect directory from deletion
      --permanent            Delete permanently (skip trash)
```

### TUI Key Bindings

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate files and groups |
| `Space` | Toggle selection / Expand group |
| `Enter` | Expand group / Preview file |
| `e` | Expand/Collapse all groups |
| `/` | Search/Filter results |
| `Tab` | Cycle sort column (Size, Path, Date, Count) |
| `v` | Cycle group filters (All, Exact, Similar) |
| `E` | Bulk select by extension |
| `D` | Bulk select by directory |
| `U` | Undo last bulk selection |
| `x` | Export results |
| `A/O/N/S/L` | Smart selection (All, Oldest, Newest, Smallest, Largest) |
| `Delete` | Delete selected files |
| `?` | Show help overlay |
| `q` or `Esc` | Quit / Go back |


## Performance

RustDupe is optimized for extreme performance through several architectural choices:

| Technique | Benefit |
|-----------|---------|
| **BLAKE3 hashing** | ~2.4 GB/s throughput on NVMe, scaling with all CPU cores. |
| **Bloom Filters** | Probabilistic rejection of unique files, reducing hashing by >80%. |
| **Parallel Walking** | `jwalk` achieves 4x faster traversal than sequential walking. |
| **Memory-Mapped I/O** | Zero-copy hashing for large files using `memmap2`. |
| **Adaptive Buffering** | I/O buffers scale from 64KB to 16MB based on file size and RAM. |
| **Work-Stealing** | Rayon-powered pipeline for maximum multi-core utilization. |

### Benchmarks (v0.3.0)

On a typical workstation (8-core CPU, NVMe SSD):

| Dataset | Files | Total Size | Time |
|---------|-------|------------|------|
| Home directory | ~150,000 | 500 GB | ~25s |
| Photo library | ~50,000 | 300 GB | ~35s |
| Large files (10GB+) | 100 | 1 TB | ~45s |

> **Note**: Performance with `--similar-images` or `--similar-documents` will be slower due to file decoding and extraction.

## Contributing

Contributions are welcome! Please read our [Contributing Guidelines](CONTRIBUTING.md) before submitting a Pull Request.

### Quick Start

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines on:
- Development setup
- Code style and linting
- Testing requirements
- Commit message conventions

## Security

For security vulnerabilities, please see our [Security Policy](SECURITY.md).

## License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.

---

<p align="center">
  Made with ❤️ in Rust
</p>
