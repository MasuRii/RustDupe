# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of the following methods:

1. **GitHub Security Advisories** (Preferred):
   - Go to the [Security Advisories](https://github.com/MasuRii/RustDupe/security/advisories) page
   - Click "New draft security advisory"
   - Fill in the details of the vulnerability

2. **Email**:
   - Send an email to the project maintainers
   - Include `[SECURITY]` in the subject line
   - Encrypt sensitive details if possible

### What to Include

Please include as much of the following information as possible:

- **Type of vulnerability** (e.g., path traversal, arbitrary code execution)
- **Affected component** (e.g., file scanner, TUI, output formatter)
- **Steps to reproduce** the issue
- **Proof of concept** code or exploit (if available)
- **Impact assessment** — what an attacker could achieve
- **Suggested fix** (if you have one)

### What to Expect

1. **Acknowledgment**: We will acknowledge receipt within 48 hours
2. **Initial Assessment**: We will provide an initial assessment within 7 days
3. **Regular Updates**: We will keep you informed of our progress
4. **Resolution**: We aim to resolve critical issues within 30 days

### Disclosure Policy

- We follow [Coordinated Vulnerability Disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure)
- We will credit reporters in the security advisory (unless you prefer to remain anonymous)
- We request that you do not publicly disclose the issue until we have released a fix

## Security Best Practices for Users

### General Recommendations

1. **Keep Updated**: Always use the latest version of RustDupe
2. **Verify Downloads**: Check SHA256 checksums of downloaded binaries
3. **Review Before Deleting**: Always review duplicate groups before confirming deletion
4. **Use Trash**: Keep the default trash behavior instead of `--permanent`

### Verifying Binary Integrity

Each release includes SHA256 checksums. Verify your download:

```bash
# Linux/macOS
sha256sum -c rustdupe-*.sha256

# Windows (PowerShell)
Get-FileHash rustdupe-*.exe | Format-List
```

### Running with Least Privilege

RustDupe only needs read access to scan directories and write access to delete files. On Unix systems, avoid running as root unless necessary.

## Security Considerations

### File Operations

- RustDupe reads file contents for hashing but never executes files
- Deletion uses the system trash by default, allowing recovery
- Symlinks are not followed by default to prevent escape attacks

### Data Handling

- No data is sent externally — RustDupe operates entirely locally
- No telemetry or analytics are collected
- Output files (JSON/CSV) are written to user-specified locations only

### Dependencies

We regularly audit our dependencies for known vulnerabilities using:

```bash
cargo audit
```

---

Thank you for helping keep RustDupe secure!
