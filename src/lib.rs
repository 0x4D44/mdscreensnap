//! mdscreensnap - A simple command-line screenshot tool for Windows and WSL

use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::Parser;
use std::path::{Path, PathBuf};

pub mod runtime;
use runtime::Runtime;

/// A simple command-line screenshot tool for Windows and WSL
#[derive(Parser, Debug, PartialEq)]
#[command(name = "mdscreensnap")]
#[command(author = "0x4D44")]
#[command(version)]
#[command(about = "Capture screenshots and save them to the temporary directory", long_about = None)]
pub struct Args {
    /// Output file path (e.g. screenshot.png). Overrides --name and --output.
    #[arg(conflicts_with_all = ["name", "output"])]
    pub file: Option<PathBuf>,

    /// Custom filename suffix (optional, will be appended after the date)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Output directory (defaults to system temp directory)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Delay in seconds before taking the screenshot
    #[arg(short, long, default_value = "0")]
    pub delay: u64,

    /// Open the screenshot after saving
    #[arg(long)]
    pub open: bool,

    /// Print the output path without capturing (dry run)
    #[arg(long)]
    pub dry_run: bool,

    /// Suppress all output (quiet mode)
    #[arg(short, long)]
    pub quiet: bool,

    /// Copy screenshot to clipboard (in addition to saving file)
    #[arg(short, long)]
    pub clipboard: bool,

    /// Don't save to file, only copy to clipboard (requires --clipboard)
    #[arg(long, requires = "clipboard")]
    pub no_save: bool,

    /// Monitor to capture: 0, 1, 2... for specific monitor, or "all" for all monitors
    #[arg(short, long, default_value = "primary")]
    pub monitor: String,

    /// List available monitors and exit
    #[arg(long)]
    pub list_monitors: bool,
}

/// Macro for conditional printing based on quiet mode
macro_rules! print_unless_quiet {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet {
            println!($($arg)*);
        }
    };
}

/// Parse monitor selection from string
pub fn parse_monitor_selection(monitor: &str) -> MonitorSelection {
    match monitor.to_lowercase().as_str() {
        "primary" => MonitorSelection::Primary,
        "all" => MonitorSelection::All,
        s => s
            .parse::<usize>()
            .map(MonitorSelection::Index)
            .unwrap_or(MonitorSelection::Primary),
    }
}

/// Monitor selection options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorSelection {
    Primary,
    Index(usize),
    All,
}

impl std::fmt::Display for MonitorSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonitorSelection::Primary => write!(f, "primary"),
            MonitorSelection::Index(i) => write!(f, "{i}"),
            MonitorSelection::All => write!(f, "all"),
        }
    }
}

/// Main application logic, separated for testing
pub fn run(args: Args, rt: &impl Runtime) -> Result<()> {
    let quiet = args.quiet;

    // Handle --list-monitors
    if args.list_monitors {
        return list_monitors(rt);
    }

    // Parse monitor selection
    let monitor = parse_monitor_selection(&args.monitor);

    // Determine output path: positional arg takes priority, otherwise auto-generate
    let output_path = if let Some(file) = args.file {
        // Explicit file path: ensure .png extension and parent directory exists
        let file = if file.extension().is_none() {
            file.with_extension("png")
        } else {
            file
        };
        let parent = file.parent().unwrap_or(Path::new("."));
        rt.create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {parent:?}"))?;
        // Resolve relative paths against current directory
        if file.is_relative() {
            std::env::current_dir()?.join(&file)
        } else {
            file
        }
    } else {
        let output_dir = args.output.unwrap_or_else(|| get_temp_dir(rt));
        rt.create_dir_all(&output_dir)
            .with_context(|| format!("Failed to create output directory: {output_dir:?}"))?;
        let filename = generate_filename(&args.name);
        output_dir.join(&filename)
    };

    if args.dry_run {
        // Always print path in dry-run mode (that's the point)
        println!("{}", output_path.display());
        return Ok(());
    }

    // Apply delay if specified
    if args.delay > 0 {
        print_unless_quiet!(quiet, "Waiting {} seconds before capture...", args.delay);
        rt.sleep(std::time::Duration::from_secs(args.delay));
    }

    // For no-save mode, use a temp file
    let (actual_output_path, is_temp) = if args.no_save {
        let temp_path = rt
            .temp_dir()
            .join(format!("mdscreensnap_temp_{}.png", std::process::id()));
        (temp_path, true)
    } else {
        (output_path.clone(), false)
    };

    // Capture the screenshot
    capture_screenshot_with_monitor(rt, &actual_output_path, &monitor)?;

    if !args.no_save {
        print_unless_quiet!(quiet, "Screenshot saved to: {}", output_path.display());
    }

    // Copy to clipboard if requested
    if args.clipboard {
        copy_to_clipboard(rt, &actual_output_path)?;
        print_unless_quiet!(quiet, "Screenshot copied to clipboard");
    }

    // Clean up temp file if no-save mode
    if is_temp {
        let _ = rt.remove_file(&actual_output_path);
    }

    // Open the screenshot if requested (only if saved)
    if args.open && !args.no_save {
        open_file(rt, &output_path)?;
    }

    Ok(())
}

/// List available monitors
pub fn list_monitors(rt: &impl Runtime) -> Result<()> {
    if rt.is_windows() {
        list_monitors_windows(rt)
    } else if is_wsl(rt) {
        list_monitors_wsl(rt)
    } else {
        bail!("Monitor listing is only supported on Windows and WSL")
    }
}

fn list_monitors_windows(rt: &impl Runtime) -> Result<()> {
    let ps_script = r#"#
    Add-Type -AssemblyName System.Windows.Forms
    $screens = [System.Windows.Forms.Screen]::AllScreens
    $i = 0
    foreach ($screen in $screens) {
        $primary = if ($screen.Primary) { " (primary)" } else { "" }
        Write-Host "${i}: $($screen.DeviceName)$primary - $($screen.Bounds.Width)x$($screen.Bounds.Height) at ($($screen.Bounds.X),$($screen.Bounds.Y))"
        $i++
    }
    "#;

    let output = rt.exec_command(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", ps_script],
    )?;
    print!("{}", String::from_utf8_lossy(&output));
    Ok(())
}

fn list_monitors_wsl(rt: &impl Runtime) -> Result<()> {
    let ps_script = r#"#
    Add-Type -AssemblyName System.Windows.Forms
    $screens = [System.Windows.Forms.Screen]::AllScreens
    $i = 0
    foreach ($screen in $screens) {
        $primary = if ($screen.Primary) { " (primary)" } else { "" }
        Write-Host "${i}: $($screen.DeviceName)$primary - $($screen.Bounds.Width)x$($screen.Bounds.Height) at ($($screen.Bounds.X),$($screen.Bounds.Y))"
        $i++
    }
    "#;

    let output = rt.exec_command(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", ps_script],
    )?;
    print!("{}", String::from_utf8_lossy(&output));
    Ok(())
}

/// Get the system temporary directory
pub fn get_temp_dir(rt: &impl Runtime) -> PathBuf {
    if is_wsl(rt) {
        // On WSL, use Windows temp directory for better compatibility
        get_windows_temp_from_wsl(rt).unwrap_or_else(|_| rt.temp_dir())
    } else {
        rt.temp_dir()
    }
}

/// Check if running under WSL by examining /proc/version content
pub fn check_wsl_from_proc_version(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("microsoft") || lower.contains("wsl")
}

/// Check if running under WSL
pub fn is_wsl(rt: &impl Runtime) -> bool {
    if rt.is_windows() {
        return false;
    }

    // Check for WSL-specific indicators
    if let Ok(content) = rt.read_to_string(Path::new("/proc/version")) {
        if check_wsl_from_proc_version(&content) {
            return true;
        }
    }

    // Check for WSL interop
    rt.path_exists(Path::new("/proc/sys/fs/binfmt_misc/WSLInterop"))
}

/// Get Windows temp directory from WSL
pub fn get_windows_temp_from_wsl(rt: &impl Runtime) -> Result<PathBuf> {
    let output = rt.exec_command("cmd.exe", &["/C", "echo %TEMP%"])?;
    let windows_temp = String::from_utf8_lossy(&output).trim().to_string();

    // Convert Windows path to WSL path
    let output = rt.exec_command("wslpath", &["-u", &windows_temp])?;
    let wsl_path = String::from_utf8_lossy(&output).trim().to_string();

    Ok(PathBuf::from(wsl_path))
}

/// Generate a filename with YYYY.MM.DD prefix using current time
pub fn generate_filename(suffix: &Option<String>) -> String {
    let now = Local::now();
    generate_filename_with_datetime(suffix, now.format("%Y.%m.%d_%H%M%S").to_string())
}

/// Generate a filename with a given date prefix (for testing)
pub fn generate_filename_with_datetime(suffix: &Option<String>, date_prefix: String) -> String {
    match suffix {
        Some(s) => format!("{}_{}.png", date_prefix, sanitize_filename(s)),
        None => format!("{date_prefix}.png"),
    }
}

/// Sanitize a filename by removing or replacing invalid characters
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Check if a character is invalid for filenames
pub fn is_invalid_filename_char(c: char) -> bool {
    matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

/// Build the full output path from directory and filename
pub fn build_output_path(output_dir: &std::path::Path, suffix: &Option<String>) -> PathBuf {
    let filename = generate_filename(suffix);
    output_dir.join(filename)
}

/// Capture a screenshot with monitor selection
pub fn capture_screenshot_with_monitor(
    rt: &impl Runtime,
    output_path: &std::path::Path,
    monitor: &MonitorSelection,
) -> Result<()> {
    if rt.is_windows() {
        capture_screenshot_windows_with_monitor(rt, output_path, monitor)
    } else if is_wsl(rt) {
        capture_screenshot_wsl_with_monitor(rt, output_path, monitor)
    } else {
        bail!("Screenshot capture is only supported on Windows and WSL")
    }
}

fn capture_screenshot_windows_with_monitor(
    rt: &impl Runtime,
    output_path: &std::path::Path,
    monitor: &MonitorSelection,
) -> Result<()> {
    // Use PowerShell for all captures - consistent PNG output
    let ps_script =
        generate_powershell_script_with_monitor(&output_path.to_string_lossy(), monitor);

    rt.exec_command(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &ps_script],
    )?;
    Ok(())
}

fn capture_screenshot_wsl_with_monitor(
    rt: &impl Runtime,
    output_path: &std::path::Path,
    monitor: &MonitorSelection,
) -> Result<()> {
    let windows_path = wsl_to_windows_path(rt, output_path)?;
    let ps_script = generate_powershell_script_with_monitor(&windows_path, monitor);

    rt.exec_command(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &ps_script],
    )?;
    Ok(())
}

/// Generate PowerShell script with monitor selection
#[allow(clippy::uninlined_format_args)]
pub fn generate_powershell_script_with_monitor(
    windows_path: &str,
    monitor: &MonitorSelection,
) -> String {
    let escaped_path = windows_path.replace('\\', "\\\\").replace('\'', "''");

    match monitor {
        MonitorSelection::Primary => {
            format!(
                r#"#
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$screen = [System.Windows.Forms.Screen]::PrimaryScreen
$bounds = $screen.Bounds
$bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

$bitmap.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)

$graphics.Dispose()
$bitmap.Dispose()
"#,
                escaped_path
            )
        }
        MonitorSelection::Index(idx) => {
            format!(
                r#"#
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$screens = [System.Windows.Forms.Screen]::AllScreens
$index = {}
if ($index -ge $screens.Length) {{
    Write-Error "Monitor index $index not found. Available: 0-$($screens.Length - 1)"
    exit 1
}}
$screen = $screens[$index]
$bounds = $screen.Bounds
$bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

$bitmap.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)

$graphics.Dispose()
$bitmap.Dispose()
"#,
                idx, escaped_path
            )
        }
        MonitorSelection::All => {
            format!(
                r#"#
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$screens = [System.Windows.Forms.Screen]::AllScreens

# Calculate the bounding rectangle for all screens
$minX = ($screens | ForEach-Object {{ $_.Bounds.X }} | Measure-Object -Minimum).Minimum
$minY = ($screens | ForEach-Object {{ $_.Bounds.Y }} | Measure-Object -Minimum).Minimum
$maxX = ($screens | ForEach-Object {{ $_.Bounds.X + $_.Bounds.Width }} | Measure-Object -Maximum).Maximum
$maxY = ($screens | ForEach-Object {{ $_.Bounds.Y + $_.Bounds.Height }} | Measure-Object -Maximum).Maximum

$totalWidth = [int]($maxX - $minX)
$totalHeight = [int]($maxY - $minY)

if ($totalWidth -le 0 -or $totalHeight -le 0) {{
    Write-Error "Invalid screen dimensions: ${{totalWidth}}x${{totalHeight}}"
    exit 1
}}

$bitmap = New-Object System.Drawing.Bitmap($totalWidth, $totalHeight)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)

# Capture each screen
foreach ($screen in $screens) {{
    $bounds = $screen.Bounds
    $offsetX = $bounds.X - $minX
    $offsetY = $bounds.Y - $minY
    $graphics.CopyFromScreen($bounds.Location, (New-Object System.Drawing.Point($offsetX, $offsetY)), $bounds.Size)
}}

$bitmap.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)

$graphics.Dispose()
$bitmap.Dispose()
"#,
                escaped_path
            )
        }
    }
}

/// Convert WSL path to Windows path
pub fn wsl_to_windows_path(rt: &impl Runtime, path: &std::path::Path) -> Result<String> {
    let output = rt.exec_command("wslpath", &["-w", &path.to_string_lossy()])?;
    Ok(String::from_utf8_lossy(&output).trim().to_string())
}

/// Copy a screenshot to the clipboard
pub fn copy_to_clipboard(rt: &impl Runtime, path: &std::path::Path) -> Result<()> {
    if rt.is_windows() {
        copy_to_clipboard_windows(rt, path)
    } else if is_wsl(rt) {
        copy_to_clipboard_wsl(rt, path)
    } else {
        bail!("Clipboard copy is only supported on Windows and WSL")
    }
}

/// Copy to clipboard on native Windows using PowerShell
fn copy_to_clipboard_windows(rt: &impl Runtime, path: &std::path::Path) -> Result<()> {
    let ps_script = format!(
        r#"#
Add-Type -AssemblyName System.Windows.Forms
$image = [System.Drawing.Image]::FromFile('{}')
[System.Windows.Forms.Clipboard]::SetImage($image)
$image.Dispose()
"#,
        path.to_string_lossy().replace('\'', "''")
    );

    rt.exec_command(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &ps_script],
    )?;
    Ok(())
}

/// Copy to clipboard from WSL using PowerShell
fn copy_to_clipboard_wsl(rt: &impl Runtime, path: &std::path::Path) -> Result<()> {
    let windows_path = wsl_to_windows_path(rt, path)?;

    let ps_script = format!(
        r#"#
Add-Type -AssemblyName System.Windows.Forms
$image = [System.Drawing.Image]::FromFile('{}')
[System.Windows.Forms.Clipboard]::SetImage($image)
$image.Dispose()
"#,
        windows_path.replace('\\', "\\\\").replace('\'', "''")
    );

    rt.exec_command(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &ps_script],
    )?;
    Ok(())
}

/// Open a file with the default application
pub fn open_file(rt: &impl Runtime, path: &std::path::Path) -> Result<()> {
    if rt.is_windows() {
        rt.spawn_command("cmd", &["/C", "start", "", &path.to_string_lossy()])?;
    } else if is_wsl(rt) {
        // Use Windows explorer from WSL
        let windows_path = wsl_to_windows_path(rt, path)?;
        rt.spawn_command("cmd.exe", &["/C", "start", "", &windows_path])?;
    } else {
        rt.spawn_command("xdg-open", &[&path.to_string_lossy()])?;
    }

    Ok(())
}

/// Generate the PowerShell script for WSL screenshot capture (primary monitor)
/// This function is kept for backward compatibility if any, or general utility
pub fn generate_powershell_script(windows_path: &str) -> String {
    generate_powershell_script_with_monitor(windows_path, &MonitorSelection::Primary)
}

/// Capture method enum (mostly for info)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    Windows,
    Wsl,
    Unsupported,
}

impl std::fmt::Display for CaptureMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureMethod::Windows => write!(f, "Windows (PowerShell)"),
            CaptureMethod::Wsl => write!(f, "WSL (PowerShell)"),
            CaptureMethod::Unsupported => write!(f, "Unsupported"),
        }
    }
}
/// Validate that a filename matches the expected date format pattern (test-only)
#[cfg(test)]
pub fn validate_filename_format(filename: &str) -> bool {
    // Pattern: YYYY.MM.DD_HHMMSS[_suffix].png
    let re_without_suffix = regex::Regex::new(r"^\d{4}\.\d{2}\.\d{2}_\d{6}\.png$").unwrap();
    let re_with_suffix = regex::Regex::new(r"^\d{4}\.\d{2}\.\d{2}_\d{6}_.+\.png$").unwrap();

    re_without_suffix.is_match(filename) || re_with_suffix.is_match(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Duration;

    // Mock Runtime Implementation
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Platform {
        Windows,
        Wsl,
        Linux,
    }

    struct MockRuntime {
        pub platform: Platform,
        pub cmds: RefCell<Vec<(String, Vec<String>)>>,
        pub spawn_cmds: RefCell<Vec<(String, Vec<String>)>>,
        pub files: RefCell<HashMap<PathBuf, String>>,
        pub file_removals: RefCell<Vec<PathBuf>>,
        pub sleeps: RefCell<Vec<Duration>>,
        pub responses: RefCell<HashMap<String, Vec<u8>>>, // Key: command name, Value: stdout
    }

    impl MockRuntime {
        fn new(platform: Platform) -> Self {
            Self {
                platform,
                cmds: RefCell::new(Vec::new()),
                spawn_cmds: RefCell::new(Vec::new()),
                files: RefCell::new(HashMap::new()),
                file_removals: RefCell::new(Vec::new()),
                sleeps: RefCell::new(Vec::new()),
                responses: RefCell::new(HashMap::new()),
            }
        }

        fn with_response(self, cmd: &str, output: &[u8]) -> Self {
            self.responses
                .borrow_mut()
                .insert(cmd.to_string(), output.to_vec());
            self
        }
    }

    impl Runtime for MockRuntime {
        fn sleep(&self, duration: Duration) {
            self.sleeps.borrow_mut().push(duration);
        }

        fn temp_dir(&self) -> PathBuf {
            PathBuf::from("/tmp")
        }

        fn create_dir_all(&self, _path: &Path) -> Result<()> {
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<()> {
            self.file_removals.borrow_mut().push(path.to_path_buf());
            Ok(())
        }

        fn read_to_string(&self, path: &Path) -> Result<String> {
            self.files
                .borrow()
                .get(path)
                .cloned()
                .context("File not found in mock")
        }

        fn path_exists(&self, path: &Path) -> bool {
            self.files.borrow().contains_key(path)
        }

        fn exec_command(&self, command: &str, args: &[&str]) -> Result<Vec<u8>> {
            let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            self.cmds.borrow_mut().push((command.to_string(), args_vec));

            if let Some(response) = self.responses.borrow().get(command) {
                return Ok(response.clone());
            }

            // Default mock responses for specific known commands to avoid failures
            if command == "wslpath" {
                // Mock wslpath implementation
                // simple mock: replace /mnt/c with C:\
                let arg = args[1];
                if arg.starts_with("/mnt/c/") {
                    Ok(arg
                        .replace("/mnt/c/", "C:\\")
                        .replace("/", "\\")
                        .as_bytes()
                        .to_vec())
                } else if arg.starts_with("C:") {
                    Ok(arg
                        .replace("C:\\", "/mnt/c/")
                        .replace("\\", "/")
                        .as_bytes()
                        .to_vec())
                } else {
                    Ok(arg.as_bytes().to_vec())
                }
            } else {
                Ok(Vec::new())
            }
        }

        fn spawn_command(&self, command: &str, args: &[&str]) -> Result<()> {
            let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            self.spawn_cmds
                .borrow_mut()
                .push((command.to_string(), args_vec));
            Ok(())
        }

        fn is_windows(&self) -> bool {
            self.platform == Platform::Windows
        }
    }

    // ==================== Tests ====================

    #[test]
    fn test_run_dry_run() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: None,
            dry_run: true,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        // Should not execute any commands
        assert!(rt.cmds.borrow().is_empty());
    }

    #[test]
    fn test_run_capture_windows_primary() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: Some("test".to_string()),
            output: Some(PathBuf::from("C:\\Screenshots")),
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, "powershell");
        assert!(cmds[0].1.contains(&"-Command".to_string()));
        // Verify script contains path and monitor logic
        let script = &cmds[0].1[3];
        assert!(script.contains("C:\\\\Screenshots"));
        assert!(script.contains("PrimaryScreen"));
    }

    #[test]
    fn test_run_capture_wsl_primary() {
        let rt =
            MockRuntime::new(Platform::Wsl).with_response("wslpath", b"C:\\Screenshots\\test.png"); // Mock wslpath output

        rt.files.borrow_mut().insert(
            PathBuf::from("/proc/version"),
            "Linux version ... Microsoft ... WSL2".to_string(),
        );

        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: None,
            output: Some(PathBuf::from("/mnt/c/Screenshots")),
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();
        // 1. wslpath
        // 2. powershell.exe
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].0, "wslpath");
        assert_eq!(cmds[1].0, "powershell.exe");
    }

    #[test]
    fn test_run_delay() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 5,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let sleeps = rt.sleeps.borrow();
        assert_eq!(sleeps.len(), 1);
        assert_eq!(sleeps[0].as_secs(), 5);
    }

    #[test]
    fn test_run_clipboard_only_no_save() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 0,
            open: false,
            quiet: false,
            clipboard: true,
            no_save: true,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();
        // 1. Capture (powershell)
        // 2. Clipboard (powershell)
        assert_eq!(cmds.len(), 2);

        // Verify capture used temp path
        let capture_script = &cmds[0].1[3];
        assert!(capture_script.contains("mdscreensnap_temp"));

        // Verify cleanup happened
        let removals = rt.file_removals.borrow();
        assert_eq!(removals.len(), 1);
        assert!(removals[0].to_string_lossy().contains("mdscreensnap_temp"));
    }

    #[test]
    fn test_run_open_file() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: None,
            output: Some(PathBuf::from("C:\\Out")),
            delay: 0,
            open: true,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let spawned = rt.spawn_cmds.borrow();
        assert_eq!(spawned.len(), 1);
        assert_eq!(spawned[0].0, "cmd");
        // Check if the last argument starts with the output directory
        assert!(spawned[0].1.iter().any(|arg| arg.starts_with("C:\\Out")));
    }

    #[test]
    fn test_list_monitors_windows() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: true,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, "powershell");
        assert!(cmds[0].1[3].contains("System.Windows.Forms.Screen"));
    }

    #[test]
    fn test_is_wsl_detection() {
        // Case 1: Windows (returns false immediately)
        let rt_win = MockRuntime::new(Platform::Windows);
        assert!(!is_wsl(&rt_win));

        // Case 2: Linux with WSL version file
        let rt_wsl = MockRuntime::new(Platform::Wsl);
        rt_wsl.files.borrow_mut().insert(
            PathBuf::from("/proc/version"),
            "Linux version ... Microsoft ... WSL2".to_string(),
        );
        assert!(is_wsl(&rt_wsl));

        // Case 3: Linux with Interop file
        let rt_interop = MockRuntime::new(Platform::Wsl);
        rt_interop.files.borrow_mut().insert(
            PathBuf::from("/proc/sys/fs/binfmt_misc/WSLInterop"),
            "".to_string(),
        );
        assert!(is_wsl(&rt_interop));

        // Case 4: Native Linux
        let rt_linux = MockRuntime::new(Platform::Linux);
        rt_linux.files.borrow_mut().insert(
            PathBuf::from("/proc/version"),
            "Linux version ... generic".to_string(),
        );
        assert!(!is_wsl(&rt_linux));
    }

    #[test]
    fn test_unsupported_platform() {
        let rt = MockRuntime::new(Platform::Linux);
        let args = Args {
            file: None,
            dry_run: false,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        // Should fail on Linux
        let result = run(args, &rt);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Screenshot capture is only supported on Windows and WSL"
        );
    }

    // Preserve original unit tests
    #[test]
    fn test_generate_filename_without_suffix() {
        let filename = generate_filename(&None);
        assert!(filename.ends_with(".png"));
        assert!(filename.contains("."));
    }

    #[test]
    fn test_generate_filename_with_suffix() {
        let filename = generate_filename(&Some("test".to_string()));
        assert!(filename.ends_with("_test.png"));
    }

    #[test]
    fn test_sanitize_filename_forward_slash() {
        assert_eq!(sanitize_filename("hello/world"), "hello_world");
    }

    #[test]
    fn test_get_temp_dir_wsl() {
        let rt = MockRuntime::new(Platform::Wsl)
            .with_response("cmd.exe", b"C:\\Users\\test\\AppData\\Local\\Temp\r\n")
            .with_response("wslpath", b"/mnt/c/Users/test/AppData/Local/Temp\n");

        rt.files
            .borrow_mut()
            .insert(PathBuf::from("/proc/version"), "Linux ... WSL2".to_string());

        let temp = get_temp_dir(&rt);
        assert_eq!(
            temp.to_string_lossy(),
            "/mnt/c/Users/test/AppData/Local/Temp"
        );
    }

    #[test]

    fn test_list_monitors_wsl() {
        let rt = MockRuntime::new(Platform::Wsl);

        rt.files
            .borrow_mut()
            .insert(PathBuf::from("/proc/version"), "Linux ... WSL2".to_string());

        let args = Args {
            file: None,
            dry_run: false,

            monitor: "primary".to_string(),

            name: None,

            output: None,

            delay: 0,

            open: false,

            quiet: false,

            clipboard: false,

            no_save: false,

            list_monitors: true,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();

        // 1. check_wsl_from_proc_version reads file, no cmd

        // 2. list_monitors_wsl runs powershell.exe

        assert!(!cmds.is_empty());

        let last_cmd = cmds.last().unwrap();

        assert_eq!(last_cmd.0, "powershell.exe");

        assert!(last_cmd.1[3].contains("System.Windows.Forms.Screen"));
    }

    #[test]

    fn test_run_capture_all() {
        let rt = MockRuntime::new(Platform::Windows);

        let args = Args {
            file: None,
            dry_run: false,

            monitor: "all".to_string(),

            name: None,

            output: Some(PathBuf::from("C:\\Screenshots")),

            delay: 0,

            open: false,

            quiet: false,

            clipboard: false,

            no_save: false,

            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();

        assert_eq!(cmds.len(), 1);

        let script = &cmds[0].1[3];

        assert!(script.contains("$totalWidth"));

        assert!(script.contains("foreach ($screen in $screens)"));
    }

    #[test]

    fn test_run_capture_index() {
        let rt = MockRuntime::new(Platform::Windows);

        let args = Args {
            file: None,
            dry_run: false,

            monitor: "1".to_string(),

            name: None,

            output: Some(PathBuf::from("C:\\Screenshots")),

            delay: 0,

            open: false,

            quiet: false,

            clipboard: false,

            no_save: false,

            list_monitors: false,
        };

        run(args, &rt).unwrap();

        let cmds = rt.cmds.borrow();

        assert_eq!(cmds.len(), 1);

        let script = &cmds[0].1[3];

        assert!(script.contains("$screens[$index]"));

        assert!(script.contains("$index = 1"));
    }

    #[test]

    fn test_capture_method_display() {
        assert_eq!(
            format!("{}", CaptureMethod::Windows),
            "Windows (PowerShell)"
        );

        assert_eq!(format!("{}", CaptureMethod::Wsl), "WSL (PowerShell)");

        assert_eq!(format!("{}", CaptureMethod::Unsupported), "Unsupported");
    }

    #[test]

    fn test_list_monitors_unsupported() {
        let rt = MockRuntime::new(Platform::Linux);

        let args = Args {
            file: None,
            dry_run: false,

            monitor: "primary".to_string(),

            name: None,

            output: None,

            delay: 0,

            open: false,

            quiet: false,

            clipboard: false,

            no_save: false,

            list_monitors: true,
        };

        let result = run(args, &rt);

        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err().to_string(),
            "Monitor listing is only supported on Windows and WSL"
        );
    }

    #[test]
    fn test_run_positional_file_arg() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: Some(PathBuf::from("C:\\Screenshots\\capture.png")),
            dry_run: true,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        run(args, &rt).unwrap();

        // dry_run just prints path, no commands executed
        assert!(rt.cmds.borrow().is_empty());
    }

    #[test]
    fn test_run_positional_file_adds_png_extension() {
        let rt = MockRuntime::new(Platform::Windows);
        let args = Args {
            file: Some(PathBuf::from("C:\\Screenshots\\capture")),
            dry_run: true,
            monitor: "primary".to_string(),
            name: None,
            output: None,
            delay: 0,
            open: false,
            quiet: false,
            clipboard: false,
            no_save: false,
            list_monitors: false,
        };

        // Should not panic - .png gets added automatically
        run(args, &rt).unwrap();
    }

    #[test]
    fn test_positional_conflicts_with_name_and_output() {
        use clap::error::ErrorKind;

        // file + --name should be rejected by clap
        let result = Args::try_parse_from(["mdscreensnap", "out.png", "--name", "foo"]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::ArgumentConflict);

        // file + --output should be rejected by clap
        let result = Args::try_parse_from(["mdscreensnap", "out.png", "--output", "dir"]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::ArgumentConflict);
    }
}
