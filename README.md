# RustDupe - Smart Duplicate File Finder

RustDupe is a high-performance, cross-platform duplicate file finder built in Rust. It utilizes the BLAKE3 hashing algorithm for fast content verification and provides an interactive TUI (Terminal User Interface) for reviewing and managing duplicate groups.

![TUI Mockup](https://raw.githubusercontent.com/rustdupe/rustdupe/master/docs/mockup.png)
*(Note: Screenshot placeholder. Run `rustdupe scan` to see the live interface.)*

## Features

- **High Performance**: Parallel directory walking and BLAKE3 hashing for maximum speed.
- **Interactive TUI**: Review duplicate groups, preview files, and select copies for deletion in a navigable interface.
- **Multi-Phase Optimization**:
  1. Group by file size (instant filtering).
  2. Compare 4KB pre-hashes (fast rejection).
  3. Full content hash for final confirmation.
  4. Optional byte-by-byte verification (paranoid mode).
- **Safe Deletion**: Moves files to system trash by default (cross-platform support).
- **Hardlink Aware**: Automatically detects and skips hardlinks (same inode) to prevent false positives.
- **Unicode Support**: Handles macOS NFD vs. Windows/Linux NFC normalization issues.
- **Machine Readable**: Export results to JSON or CSV for scripting and automation.

## Installation

### From Source (Requires Rust 1.75+)

```bash
cargo install rustdupe
```

### Pre-built Binaries

Download the latest release for your platform from the [GitHub Releases](https://github.com/rustdupe/rustdupe/releases) page.

## Usage

### Basic Scan (Interactive TUI)

```bash
rustdupe scan ~/Downloads
```

### Non-Interactive Modes (Automation)

```bash
# Export to JSON
rustdupe scan ~/Documents --output json > duplicates.json

# Export to CSV
rustdupe scan /path/to/media --output csv > duplicates.csv
```

### Advanced Options

```bash
# Filter by size
rustdupe scan . --min-size 1MB --max-size 1GB

# Ignore specific patterns
rustdupe scan . --ignore "*.tmp" --ignore "node_modules"

# Enable paranoid byte-by-byte verification
rustdupe scan . --paranoid

# Custom I/O threads (default: 4)
rustdupe scan . --io-threads 8
```

## CLI Reference

```text
Usage: rustdupe [OPTIONS] <COMMAND>

Commands:
  scan  Scan a directory for duplicate files
  help  Print this message or the help of the given subcommand(s)

Arguments:
  <PATH>  Directory path to scan for duplicates

Options:
  -v, --verbose...       Increase verbosity level (-v for debug, -vv for trace)
  -q, --quiet            Suppress all output except errors
      --no-color         Disable colored output
  -h, --help             Print help
  -V, --version          Print version

Scan Subcommand Options:
  -o, --output <OUTPUT>  Output format (tui, json, csv) [default: tui]
      --min-size <SIZE>  Minimum file size to consider (e.g., 1KB, 1MB)
      --max-size <SIZE>  Maximum file size to consider (e.g., 1KB, 1MB)
  -i, --ignore <PATTERN> Glob patterns to ignore
      --follow-symlinks  Follow symbolic links
      --skip-hidden      Skip hidden files and directories
      --io-threads <N>   Number of I/O threads for hashing [default: 4]
      --paranoid         Enable byte-by-byte verification
      --permanent        Use permanent deletion instead of trash
  -y, --yes              Skip confirmation prompts
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'feat: Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

Distributed under the MIT License. See `LICENSE` for more information.
