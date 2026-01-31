# Contributing to RustDupe

Thank you for your interest in contributing to RustDupe! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Code Style](#code-style)
- [Testing](#testing)
- [Commit Messages](#commit-messages)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

## Getting Started

### Prerequisites

- **Rust 1.75 or later** — Install via [rustup](https://rustup.rs/)
- **Git** — For version control
- **pre-commit** (optional) — For automated pre-commit hooks

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally:

```bash
git clone https://github.com/YOUR_USERNAME/rustdupe.git
cd rustdupe
```

3. Add the upstream remote:

```bash
git remote add upstream https://github.com/rustdupe/rustdupe.git
```

## Development Setup

### Install Dependencies

```bash
# Build the project
cargo build

# Run tests to verify setup
cargo test

# Run the application
cargo run -- scan .
```

### Optional: Set Up Pre-commit Hooks

We use pre-commit hooks to ensure code quality before commits:

```bash
# Install pre-commit (requires Python)
pip install pre-commit

# Install the hooks
pre-commit install

# Run hooks manually on all files
pre-commit run --all-files
```

### IDE Setup

**VS Code** (recommended):
- Install the [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension
- The project includes `.vscode/` settings for consistent formatting

**Other IDEs**:
- Ensure your IDE uses `rustfmt` for formatting
- Configure to use the project's `rustfmt.toml` and `clippy.toml`

## Making Changes

### Branch Naming

Use descriptive branch names with a prefix:

| Prefix | Use Case |
|--------|----------|
| `feature/` | New features |
| `fix/` | Bug fixes |
| `docs/` | Documentation changes |
| `refactor/` | Code refactoring |
| `test/` | Test additions/modifications |
| `chore/` | Maintenance tasks |

Example: `feature/add-regex-ignore-patterns`

### Workflow

1. Sync with upstream:

```bash
git fetch upstream
git checkout master
git merge upstream/master
```

2. Create a feature branch:

```bash
git checkout -b feature/your-feature-name
```

3. Make your changes with clear, atomic commits

4. Push to your fork:

```bash
git push origin feature/your-feature-name
```

5. Open a Pull Request

## Code Style

### Formatting

All code must be formatted with `rustfmt`:

```bash
# Format code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

### Linting

All code must pass `clippy` without warnings:

```bash
# Run clippy
cargo clippy -- -D warnings

# With all pedantic lints (optional, for thorough review)
cargo clippy -- -W clippy::pedantic -D warnings
```

### Style Guidelines

- **Line length**: Maximum 100 characters (configured in `rustfmt.toml`)
- **Indentation**: 4 spaces (no tabs)
- **Imports**: Grouped and sorted alphabetically
- **Documentation**: Use `///` for public APIs, `//` for implementation comments
- **Error handling**: Use `anyhow` for applications, `thiserror` for library errors
- **Naming conventions**:
  - `snake_case` for functions, variables, and modules
  - `PascalCase` for types and traits
  - `SCREAMING_SNAKE_CASE` for constants

### Documentation

- All public APIs must have documentation comments
- Include examples in doc comments where helpful
- Keep comments focused on "why" rather than "what"

```rust
/// Calculates the BLAKE3 hash of a file's contents.
///
/// This function reads the file in chunks to handle large files
/// efficiently without loading the entire file into memory.
///
/// # Arguments
///
/// * `path` - The path to the file to hash
///
/// # Returns
///
/// Returns the hex-encoded BLAKE3 hash string.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn hash_file(path: &Path) -> Result<String> {
    // ...
}
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests in a specific module
cargo test module_name::
```

### Test Guidelines

- Write tests for all new functionality
- Maintain or improve code coverage
- Use descriptive test names: `test_hash_file_returns_error_for_missing_file`
- Group related tests in modules
- Use `proptest` for property-based testing where appropriate

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptive_name() {
        // Arrange
        let input = ...;

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

## Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/) specification:

### Format

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no code change |
| `refactor` | Code restructuring |
| `perf` | Performance improvement |
| `test` | Adding/updating tests |
| `chore` | Maintenance tasks |
| `ci` | CI/CD changes |

### Examples

```
feat(scanner): add regex pattern support for ignore rules

fix(tui): prevent crash when terminal is resized during scan

docs(readme): add installation instructions for Windows

refactor(hasher): extract common hashing logic into trait
```

### Guidelines

- Use imperative mood: "add feature" not "added feature"
- First line: max 72 characters
- Body: wrap at 72 characters, explain "what" and "why"
- Reference issues: `Fixes #123` or `Closes #456`

## Pull Request Process

### Before Submitting

Ensure your PR:

- [ ] Passes all tests (`cargo test`)
- [ ] Passes clippy (`cargo clippy -- -D warnings`)
- [ ] Is formatted (`cargo fmt -- --check`)
- [ ] Has updated documentation if needed
- [ ] Has a clear, descriptive title
- [ ] References related issues

### PR Description

Use the PR template and include:

- **Summary**: What does this PR do?
- **Motivation**: Why is this change needed?
- **Changes**: List of specific changes
- **Testing**: How was this tested?
- **Screenshots**: If UI changes are involved

### Review Process

1. A maintainer will review your PR
2. Address any requested changes
3. Once approved, a maintainer will merge the PR
4. Delete your branch after merging

### Tips for Faster Reviews

- Keep PRs focused and small (< 400 lines when possible)
- Respond to feedback promptly
- Be open to suggestions
- Test edge cases

## Release Process

Releases are automated via GitHub Actions when a tag is pushed:

```bash
# Create and push a release tag
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

The release workflow will:
1. Build binaries for all supported platforms
2. Create a GitHub Release
3. Publish to crates.io (maintainers only)

---

Thank you for contributing to RustDupe! 🎉
