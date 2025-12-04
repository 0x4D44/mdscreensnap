# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build release binary
cargo build --release

# Run tests
cargo test

# Run a single test
cargo test test_name

# Check compilation without building
cargo check

# Run with clippy lints
cargo clippy
```

## Architecture

This is a single-binary Rust CLI tool for capturing screenshots on Windows and WSL. The entire implementation lives in `src/main.rs`.

### Platform Support

- **Native Windows**: Uses PowerShell with .NET `System.Drawing` for PNG capture
- **WSL**: Detects WSL environment via `/proc/version` and uses `powershell.exe` for capture

### Key Components

- `Args` struct with clap derive macro handles CLI argument parsing
- `MonitorSelection` enum: `Primary`, `Index(usize)`, `All`
- `CaptureMethod` enum: `Windows`, `Wsl`, `Unsupported`
- `run()` is the main entry point (separated from `main()` for testability)

### Capture Flow

1. Parse CLI args → determine monitor selection
2. Build output path with timestamp filename (`YYYY.MM.DD_HHMMSS[_suffix].png`)
3. For no-save mode, use temp file
4. Call appropriate capture function based on platform
5. Optionally copy to clipboard via PowerShell
6. Clean up temp files if no-save mode

### Platform Detection

- `is_wsl()`: Checks `/proc/version` for "microsoft" or "wsl", plus WSLInterop file
- `get_capture_method()`: Returns appropriate capture method for current platform

### Path Handling

- `wsl_to_windows_path()`: Uses `wslpath -w` to convert paths for PowerShell
- `get_windows_temp_from_wsl()`: Gets Windows TEMP via `cmd.exe` and converts with `wslpath -u`

### Tests

Tests are inline in `main.rs` under `#[cfg(test)]`. The test suite covers:
- Filename generation and sanitization
- CLI argument parsing
- WSL detection logic
- PowerShell script generation
- Monitor selection parsing

Uses `tempfile` crate for integration tests that create temporary directories.
