# mdscreensnap

A simple, cross-platform command-line screenshot tool for Windows and WSL (Windows Subsystem for Linux).

## Features

-   **Cross-Platform:** Works seamlessly on Windows (natively) and inside WSL (calling out to Windows PowerShell).
-   **Monitor Selection:** Capture specific monitors by index or capture all monitors at once.
-   **Custom Filenames:** Add custom suffixes to the automatically generated timestamped filenames.
-   **Clipboard Support:** Copy screenshots directly to the clipboard (with or without saving to a file).
-   **Delayed Capture:** Set a delay in seconds before taking the screenshot.
-   **Auto-Open:** Automatically open the captured screenshot in the default image viewer.
-   **Output Directory:** Specify a custom output directory or default to the system temp folder.
-   **Dry Run:** Preview the generated output path without taking a screenshot.
-   **Quiet Mode:** Suppress output for scripting usage.

## Installation

### From Source

Ensure you have Rust and Cargo installed.

```bash
git clone https://github.com/0x4D44/mdscreensnap.git
cd mdscreensnap
cargo install --path .
```

## Usage

```bash
mdscreensnap [OPTIONS]
```

### Options

| Option | Short | Description |
| :--- | :---: | :--- |
| `--name <NAME>` | `-n` | Custom filename suffix (appended after the date). |
| `--output <PATH>` | `-o` | Output directory (defaults to system temp directory). |
| `--delay <SECONDS>` | `-d` | Delay in seconds before taking the screenshot. |
| `--monitor <SELECTION>` | `-m` | Monitor to capture: `primary`, `all`, or index `0`, `1`... (Default: `primary`). |
| `--clipboard` | `-c` | Copy screenshot to clipboard. |
| `--no-save` | | Don't save to file, only copy to clipboard (requires `--clipboard`). |
| `--open` | | Open the screenshot after saving. |
| `--list-monitors` | | List available monitors and exit. |
| `--dry-run` | | Print the output path without capturing. |
| `--quiet` | `-q` | Suppress all output. |
| `--help` | `-h` | Print help information. |
| `--version` | `-V` | Print version information. |

### Examples

**Capture primary monitor to temp dir:**
```bash
mdscreensnap
```

**Capture with 5s delay and open immediately:**
```bash
mdscreensnap --delay 5 --open
```

**Capture all monitors to a specific folder with a custom name:**
```bash
mdscreensnap --monitor all --output ~/Pictures/Screenshots --name "project-update"
```
*Result:* `~/Pictures/Screenshots/2026.01.03_120000_project-update.png`

**Copy to clipboard only (no file saved):**
```bash
mdscreensnap --clipboard --no-save
```

**List available monitors:**
```bash
mdscreensnap --list-monitors
```

## Development

This project uses a `Runtime` trait to abstract system side effects (filesystem, time, process execution), making the core logic highly testable.

### Running Tests

```bash
cargo test
```

### Architecture

-   **`src/lib.rs`**: Core application logic and argument parsing.
-   **`src/main.rs`**: Thin entry point that injects the `RealRuntime`.
-   **`src/runtime.rs`**: `Runtime` trait definition and `RealRuntime` implementation (wraps `std::fs`, `std::process::Command`, etc.).

### Coverage

The project maintains high code coverage (>98% logic coverage). You can verify this using `cargo-llvm-cov`:

```bash
cargo llvm-cov
```

## License

MIT License