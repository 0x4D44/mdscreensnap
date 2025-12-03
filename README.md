# mdscreensnap

A simple command-line screenshot tool for Windows and WSL.

## Features

- Cross-platform support for Windows (native) and WSL
- Automatic date-based file naming (YYYY.MM.DD_HHMMSS format)
- Saves to system temporary directory by default
- Optional delay before capture
- Custom output directory support
- Copy to clipboard support
- Open screenshot after capture
- Quiet mode for scripting

## Installation

### From source

```bash
# Clone the repository
git clone https://github.com/0x4D44/mdscreensnap.git
cd mdscreensnap

# Build the release binary
cargo build --release

# The binary will be at target/release/mdscreensnap.exe (Windows) or target/release/mdscreensnap (Linux/WSL)
```

## Usage

```bash
# Take a screenshot immediately (saves to temp directory)
mdscreensnap

# Take a screenshot with a custom name suffix
mdscreensnap --name my-screenshot

# Take a screenshot to a specific directory
mdscreensnap --output ~/screenshots

# Take a screenshot with a 3-second delay
mdscreensnap --delay 3

# Take a screenshot and open it immediately
mdscreensnap --open

# Take a screenshot and copy to clipboard
mdscreensnap --clipboard

# Show where the screenshot would be saved (dry run)
mdscreensnap --dry-run

# Silent mode for scripting (only outputs path on dry-run)
mdscreensnap --quiet

# Combine options
mdscreensnap --name feature-demo --delay 5 --open --clipboard

# Use in scripts (get path, capture silently)
SCREENSHOT_PATH=$(mdscreensnap --dry-run)
mdscreensnap -q -c  # Capture silently with clipboard
```

## Options

| Option | Short | Description |
|--------|-------|-------------|
| `--name` | `-n` | Custom filename suffix (appended after the date) |
| `--output` | `-o` | Output directory (defaults to system temp directory) |
| `--delay` | `-d` | Delay in seconds before taking the screenshot (default: 0) |
| `--open` | | Open the screenshot after saving |
| `--clipboard` | `-c` | Copy screenshot to clipboard (in addition to file) |
| `--quiet` | `-q` | Suppress all output (quiet mode for scripting) |
| `--dry-run` | | Print the output path without capturing |
| `--help` | `-h` | Show help information |
| `--version` | `-V` | Show version information |

## File Naming

Screenshots are automatically named with the following format:

```
YYYY.MM.DD_HHMMSS[_suffix].png
```

Examples:
- `2025.12.03_143052.png` (without suffix)
- `2025.12.03_143052_my-screenshot.png` (with `--name my-screenshot`)

## Platform Support

### Windows (Native)

On native Windows, the tool uses Win32 GDI API to capture the primary screen.

### WSL (Windows Subsystem for Linux)

When running under WSL, the tool:
1. Detects the WSL environment automatically
2. Uses PowerShell to capture the Windows desktop
3. Saves to the Windows temp directory by default for better compatibility
4. Supports path conversion between WSL and Windows

## Requirements

- **Windows**: No additional requirements
- **WSL**: PowerShell must be accessible (default in WSL2)

## License

MIT License - see [LICENSE](LICENSE) for details.
