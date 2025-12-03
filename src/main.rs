//! mdscreensnap - A simple command-line screenshot tool for Windows and WSL

use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::Parser;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A simple command-line screenshot tool for Windows and WSL
#[derive(Parser, Debug, PartialEq)]
#[command(name = "mdscreensnap")]
#[command(author = "0x4D44")]
#[command(version)]
#[command(about = "Capture screenshots and save them to the temporary directory", long_about = None)]
pub struct Args {
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
}

fn main() -> Result<()> {
    let args = Args::parse();
    run(args)
}

/// Main application logic, separated for testing
pub fn run(args: Args) -> Result<()> {
    // Determine the output directory
    let output_dir = args.output.unwrap_or_else(get_temp_dir);

    // Ensure output directory exists
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("Failed to create output directory: {:?}", output_dir))?;

    // Generate filename with YYYY.MM.DD prefix
    let filename = generate_filename(&args.name);
    let output_path = output_dir.join(&filename);

    if args.dry_run {
        println!("Would save screenshot to: {}", output_path.display());
        return Ok(());
    }

    // Apply delay if specified
    if args.delay > 0 {
        println!("Waiting {} seconds before capture...", args.delay);
        std::thread::sleep(std::time::Duration::from_secs(args.delay));
    }

    // Capture the screenshot
    capture_screenshot(&output_path)?;

    println!("Screenshot saved to: {}", output_path.display());

    // Open the screenshot if requested
    if args.open {
        open_file(&output_path)?;
    }

    Ok(())
}

/// Get the system temporary directory
pub fn get_temp_dir() -> PathBuf {
    if is_wsl() {
        // On WSL, use Windows temp directory for better compatibility
        get_windows_temp_from_wsl().unwrap_or_else(|_| env::temp_dir())
    } else {
        env::temp_dir()
    }
}

/// Check if running under WSL by examining /proc/version content
pub fn check_wsl_from_proc_version(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("microsoft") || lower.contains("wsl")
}

/// Check if the WSL interop file exists
pub fn check_wsl_interop_exists() -> bool {
    std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
}

/// Check if running under WSL
pub fn is_wsl() -> bool {
    if cfg!(windows) {
        return false;
    }

    // Check for WSL-specific indicators
    if let Ok(content) = fs::read_to_string("/proc/version") {
        if check_wsl_from_proc_version(&content) {
            return true;
        }
    }

    // Check for WSL interop
    check_wsl_interop_exists()
}

/// Get Windows temp directory from WSL
pub fn get_windows_temp_from_wsl() -> Result<PathBuf> {
    let output = Command::new("cmd.exe")
        .args(["/C", "echo %TEMP%"])
        .output()
        .context("Failed to get Windows TEMP directory")?;

    let windows_temp = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Convert Windows path to WSL path
    let output = Command::new("wslpath")
        .arg("-u")
        .arg(&windows_temp)
        .output()
        .context("Failed to convert Windows path to WSL path")?;

    let wsl_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

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
        None => format!("{}.png", date_prefix),
    }
}

/// Validate that a filename matches the expected date format pattern
pub fn validate_filename_format(filename: &str) -> bool {
    // Pattern: YYYY.MM.DD_HHMMSS[_suffix].png
    let re_without_suffix = regex::Regex::new(r"^\d{4}\.\d{2}\.\d{2}_\d{6}\.png$").unwrap();
    let re_with_suffix = regex::Regex::new(r"^\d{4}\.\d{2}\.\d{2}_\d{6}_.+\.png$").unwrap();

    re_without_suffix.is_match(filename) || re_with_suffix.is_match(filename)
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
pub fn build_output_path(output_dir: &PathBuf, suffix: &Option<String>) -> PathBuf {
    let filename = generate_filename(suffix);
    output_dir.join(filename)
}

/// Capture a screenshot and save it to the specified path
pub fn capture_screenshot(output_path: &PathBuf) -> Result<()> {
    if cfg!(windows) {
        capture_screenshot_windows(output_path)
    } else if is_wsl() {
        capture_screenshot_wsl(output_path)
    } else {
        bail!("Screenshot capture is only supported on Windows and WSL")
    }
}

/// Determine the capture method based on platform
pub fn get_capture_method() -> CaptureMethod {
    if cfg!(windows) {
        CaptureMethod::Windows
    } else if is_wsl() {
        CaptureMethod::Wsl
    } else {
        CaptureMethod::Unsupported
    }
}

/// Enum representing available capture methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    Windows,
    Wsl,
    Unsupported,
}

impl std::fmt::Display for CaptureMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureMethod::Windows => write!(f, "Windows (GDI)"),
            CaptureMethod::Wsl => write!(f, "WSL (PowerShell)"),
            CaptureMethod::Unsupported => write!(f, "Unsupported"),
        }
    }
}

/// Capture screenshot on native Windows
#[cfg(windows)]
fn capture_screenshot_windows(output_path: &PathBuf) -> Result<()> {
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        // Get the screen dimensions
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        // Get the device context for the entire screen
        let screen_dc = GetDC(HWND(std::ptr::null_mut()));
        if screen_dc.is_invalid() {
            bail!("Failed to get screen device context");
        }

        // Create a compatible DC and bitmap
        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_invalid() {
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
            bail!("Failed to create compatible DC");
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, screen_width, screen_height);
        if bitmap.is_invalid() {
            DeleteDC(mem_dc);
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
            bail!("Failed to create compatible bitmap");
        }

        let old_bitmap = SelectObject(mem_dc, bitmap);

        // Copy the screen to the bitmap
        let result = BitBlt(
            mem_dc,
            0,
            0,
            screen_width,
            screen_height,
            screen_dc,
            0,
            0,
            SRCCOPY,
        );

        if !result.as_bool() {
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
            bail!("Failed to capture screen");
        }

        // Get bitmap info
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: screen_width,
                biHeight: -screen_height, // Negative for top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default()],
        };

        // Calculate buffer size and allocate
        let row_size = ((screen_width * 4 + 3) & !3) as usize;
        let buffer_size = row_size * screen_height as usize;
        let mut buffer = vec![0u8; buffer_size];

        // Get the bitmap bits
        let result = GetDIBits(
            mem_dc,
            bitmap,
            0,
            screen_height as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        );

        // Clean up GDI objects
        SelectObject(mem_dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
        ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);

        if result == 0 {
            bail!("Failed to get bitmap bits");
        }

        // Convert BGRA to RGBA
        for i in (0..buffer.len()).step_by(4) {
            buffer.swap(i, i + 2);
        }

        // Write as PNG (simple BMP for now, or use image crate)
        write_png(output_path, &buffer, screen_width as u32, screen_height as u32)?;

        Ok(())
    }
}

#[cfg(windows)]
fn write_png(path: &PathBuf, data: &[u8], width: u32, height: u32) -> Result<()> {
    use std::io::Write;

    // Write as BMP since we don't have image crate on Windows target
    let file_size = 54 + data.len() as u32;
    let mut file = fs::File::create(path.with_extension("bmp"))?;

    // BMP Header
    file.write_all(&[0x42, 0x4D])?; // BM
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(&[0, 0, 0, 0])?; // Reserved
    file.write_all(&54u32.to_le_bytes())?; // Offset to pixel data

    // DIB Header
    file.write_all(&40u32.to_le_bytes())?; // Header size
    file.write_all(&width.to_le_bytes())?;
    file.write_all(&(-(height as i32)).to_le_bytes())?; // Negative for top-down
    file.write_all(&1u16.to_le_bytes())?; // Planes
    file.write_all(&32u16.to_le_bytes())?; // Bits per pixel
    file.write_all(&0u32.to_le_bytes())?; // Compression
    file.write_all(&(data.len() as u32).to_le_bytes())?; // Image size
    file.write_all(&0u32.to_le_bytes())?; // X pixels per meter
    file.write_all(&0u32.to_le_bytes())?; // Y pixels per meter
    file.write_all(&0u32.to_le_bytes())?; // Colors used
    file.write_all(&0u32.to_le_bytes())?; // Important colors

    // Convert RGBA back to BGRA for BMP
    let mut bgra_data = data.to_vec();
    for i in (0..bgra_data.len()).step_by(4) {
        bgra_data.swap(i, i + 2);
    }
    file.write_all(&bgra_data)?;

    // Update the output path to use .bmp extension
    println!("Note: Saved as BMP format");

    Ok(())
}

#[cfg(not(windows))]
fn capture_screenshot_windows(_output_path: &PathBuf) -> Result<()> {
    bail!("Windows screenshot capture is not available on this platform")
}

/// Generate the PowerShell script for WSL screenshot capture
pub fn generate_powershell_script(windows_path: &str) -> String {
    format!(
        r#"
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

Write-Host 'Screenshot captured successfully'
"#,
        windows_path.replace('\\', "\\\\").replace('\'', "''")
    )
}

/// Capture screenshot from WSL using PowerShell
pub fn capture_screenshot_wsl(output_path: &PathBuf) -> Result<()> {
    // Convert WSL path to Windows path for PowerShell
    let windows_path = wsl_to_windows_path(output_path)?;

    // PowerShell script to capture the screen
    let ps_script = generate_powershell_script(&windows_path);

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .output()
        .context("Failed to execute PowerShell for screenshot capture")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("PowerShell screenshot failed: {}", stderr);
    }

    Ok(())
}

/// Convert WSL path to Windows path
pub fn wsl_to_windows_path(path: &PathBuf) -> Result<String> {
    let output = Command::new("wslpath")
        .args(["-w", &path.to_string_lossy()])
        .output()
        .context("Failed to convert WSL path to Windows path")?;

    if !output.status.success() {
        bail!("wslpath conversion failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Open a file with the default application
pub fn open_file(path: &PathBuf) -> Result<()> {
    if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn()
            .context("Failed to open file")?;
    } else if is_wsl() {
        // Use Windows explorer from WSL
        let windows_path = wsl_to_windows_path(path)?;
        Command::new("cmd.exe")
            .args(["/C", "start", "", &windows_path])
            .spawn()
            .context("Failed to open file")?;
    } else {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .context("Failed to open file")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::fs;
    use tempfile::tempdir;

    // ==================== Filename Generation Tests ====================

    #[test]
    fn test_generate_filename_without_suffix() {
        let filename = generate_filename(&None);
        assert!(filename.ends_with(".png"));
        assert!(filename.contains('.'));
    }

    #[test]
    fn test_generate_filename_with_suffix() {
        let filename = generate_filename(&Some("test".to_string()));
        assert!(filename.ends_with("_test.png"));
    }

    #[test]
    fn test_generate_filename_with_datetime_no_suffix() {
        let filename = generate_filename_with_datetime(&None, "2025.12.03_143052".to_string());
        assert_eq!(filename, "2025.12.03_143052.png");
    }

    #[test]
    fn test_generate_filename_with_datetime_with_suffix() {
        let filename = generate_filename_with_datetime(
            &Some("my-screenshot".to_string()),
            "2025.12.03_143052".to_string(),
        );
        assert_eq!(filename, "2025.12.03_143052_my-screenshot.png");
    }

    #[test]
    fn test_generate_filename_format_matches_pattern() {
        let filename = generate_filename(&None);
        // Should match YYYY.MM.DD_HHMMSS.png
        let parts: Vec<&str> = filename.split('.').collect();
        assert_eq!(parts.len(), 4); // YYYY, MM, DD_HHMMSS, png
        assert_eq!(parts[0].len(), 4); // Year
        assert_eq!(parts[1].len(), 2); // Month
        assert!(parts[2].contains('_')); // DD_HHMMSS
        assert_eq!(parts[3], "png");
    }

    #[test]
    fn test_generate_filename_with_special_chars_in_suffix() {
        let filename = generate_filename_with_datetime(
            &Some("test/file:name".to_string()),
            "2025.12.03_143052".to_string(),
        );
        assert_eq!(filename, "2025.12.03_143052_test_file_name.png");
    }

    // ==================== Filename Sanitization Tests ====================

    #[test]
    fn test_sanitize_filename_forward_slash() {
        assert_eq!(sanitize_filename("hello/world"), "hello_world");
    }

    #[test]
    fn test_sanitize_filename_backslash() {
        assert_eq!(sanitize_filename("hello\\world"), "hello_world");
    }

    #[test]
    fn test_sanitize_filename_colon() {
        assert_eq!(sanitize_filename("test:file"), "test_file");
    }

    #[test]
    fn test_sanitize_filename_asterisk() {
        assert_eq!(sanitize_filename("file*name"), "file_name");
    }

    #[test]
    fn test_sanitize_filename_question_mark() {
        assert_eq!(sanitize_filename("file?name"), "file_name");
    }

    #[test]
    fn test_sanitize_filename_quotes() {
        assert_eq!(sanitize_filename("file\"name"), "file_name");
    }

    #[test]
    fn test_sanitize_filename_angle_brackets() {
        assert_eq!(sanitize_filename("file<>name"), "file__name");
    }

    #[test]
    fn test_sanitize_filename_pipe() {
        assert_eq!(sanitize_filename("file|name"), "file_name");
    }

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("normal-file_name.txt"), "normal-file_name.txt");
    }

    #[test]
    fn test_sanitize_filename_empty() {
        assert_eq!(sanitize_filename(""), "");
    }

    #[test]
    fn test_sanitize_filename_all_invalid() {
        assert_eq!(sanitize_filename("/<>:\"\\|?*"), "_________");
    }

    #[test]
    fn test_sanitize_filename_unicode() {
        assert_eq!(sanitize_filename("文件名"), "文件名");
    }

    #[test]
    fn test_sanitize_filename_spaces() {
        assert_eq!(sanitize_filename("file with spaces"), "file with spaces");
    }

    // ==================== Invalid Character Detection Tests ====================

    #[test]
    fn test_is_invalid_filename_char_slash() {
        assert!(is_invalid_filename_char('/'));
        assert!(is_invalid_filename_char('\\'));
    }

    #[test]
    fn test_is_invalid_filename_char_special() {
        assert!(is_invalid_filename_char(':'));
        assert!(is_invalid_filename_char('*'));
        assert!(is_invalid_filename_char('?'));
        assert!(is_invalid_filename_char('"'));
        assert!(is_invalid_filename_char('<'));
        assert!(is_invalid_filename_char('>'));
        assert!(is_invalid_filename_char('|'));
    }

    #[test]
    fn test_is_invalid_filename_char_valid() {
        assert!(!is_invalid_filename_char('a'));
        assert!(!is_invalid_filename_char('Z'));
        assert!(!is_invalid_filename_char('0'));
        assert!(!is_invalid_filename_char('-'));
        assert!(!is_invalid_filename_char('_'));
        assert!(!is_invalid_filename_char('.'));
        assert!(!is_invalid_filename_char(' '));
    }

    // ==================== Filename Validation Tests ====================

    #[test]
    fn test_validate_filename_format_valid_no_suffix() {
        assert!(validate_filename_format("2025.12.03_143052.png"));
    }

    #[test]
    fn test_validate_filename_format_valid_with_suffix() {
        assert!(validate_filename_format("2025.12.03_143052_my-screenshot.png"));
    }

    #[test]
    fn test_validate_filename_format_invalid_no_extension() {
        assert!(!validate_filename_format("2025.12.03_143052"));
    }

    #[test]
    fn test_validate_filename_format_invalid_wrong_extension() {
        assert!(!validate_filename_format("2025.12.03_143052.jpg"));
    }

    #[test]
    fn test_validate_filename_format_invalid_date() {
        assert!(!validate_filename_format("25.12.03_143052.png"));
    }

    #[test]
    fn test_validate_filename_format_random_string() {
        assert!(!validate_filename_format("random_file.png"));
    }

    // ==================== WSL Detection Tests ====================

    #[test]
    fn test_check_wsl_from_proc_version_microsoft() {
        assert!(check_wsl_from_proc_version(
            "Linux version 5.15.0-1-Microsoft (oe-user@oe-host)"
        ));
    }

    #[test]
    fn test_check_wsl_from_proc_version_wsl() {
        assert!(check_wsl_from_proc_version(
            "Linux version 5.15.0-1-WSL2 (oe-user@oe-host)"
        ));
    }

    #[test]
    fn test_check_wsl_from_proc_version_lowercase() {
        assert!(check_wsl_from_proc_version("linux wsl2 kernel"));
    }

    #[test]
    fn test_check_wsl_from_proc_version_uppercase() {
        assert!(check_wsl_from_proc_version("MICROSOFT WSL"));
    }

    #[test]
    fn test_check_wsl_from_proc_version_normal_linux() {
        assert!(!check_wsl_from_proc_version(
            "Linux version 5.15.0-generic (buildd@lcy02-amd64-086)"
        ));
    }

    #[test]
    fn test_check_wsl_from_proc_version_empty() {
        assert!(!check_wsl_from_proc_version(""));
    }

    #[test]
    fn test_check_wsl_from_proc_version_ubuntu() {
        assert!(!check_wsl_from_proc_version(
            "Linux version 5.4.0-42-generic #46-Ubuntu SMP Fri Jul 10 00:24:02 UTC 2020"
        ));
    }

    // ==================== CLI Argument Parsing Tests ====================

    #[test]
    fn test_args_parse_empty() {
        let args = Args::try_parse_from(["mdscreensnap"]).unwrap();
        assert_eq!(args.name, None);
        assert_eq!(args.output, None);
        assert_eq!(args.delay, 0);
        assert!(!args.open);
        assert!(!args.dry_run);
    }

    #[test]
    fn test_args_parse_name_short() {
        let args = Args::try_parse_from(["mdscreensnap", "-n", "test"]).unwrap();
        assert_eq!(args.name, Some("test".to_string()));
    }

    #[test]
    fn test_args_parse_name_long() {
        let args = Args::try_parse_from(["mdscreensnap", "--name", "my-screenshot"]).unwrap();
        assert_eq!(args.name, Some("my-screenshot".to_string()));
    }

    #[test]
    fn test_args_parse_output_short() {
        let args = Args::try_parse_from(["mdscreensnap", "-o", "/tmp/screenshots"]).unwrap();
        assert_eq!(args.output, Some(PathBuf::from("/tmp/screenshots")));
    }

    #[test]
    fn test_args_parse_output_long() {
        let args = Args::try_parse_from(["mdscreensnap", "--output", "/home/user/pics"]).unwrap();
        assert_eq!(args.output, Some(PathBuf::from("/home/user/pics")));
    }

    #[test]
    fn test_args_parse_delay_short() {
        let args = Args::try_parse_from(["mdscreensnap", "-d", "5"]).unwrap();
        assert_eq!(args.delay, 5);
    }

    #[test]
    fn test_args_parse_delay_long() {
        let args = Args::try_parse_from(["mdscreensnap", "--delay", "10"]).unwrap();
        assert_eq!(args.delay, 10);
    }

    #[test]
    fn test_args_parse_open() {
        let args = Args::try_parse_from(["mdscreensnap", "--open"]).unwrap();
        assert!(args.open);
    }

    #[test]
    fn test_args_parse_dry_run() {
        let args = Args::try_parse_from(["mdscreensnap", "--dry-run"]).unwrap();
        assert!(args.dry_run);
    }

    #[test]
    fn test_args_parse_all_options() {
        let args = Args::try_parse_from([
            "mdscreensnap",
            "--name",
            "test",
            "--output",
            "/tmp",
            "--delay",
            "3",
            "--open",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(args.name, Some("test".to_string()));
        assert_eq!(args.output, Some(PathBuf::from("/tmp")));
        assert_eq!(args.delay, 3);
        assert!(args.open);
        assert!(args.dry_run);
    }

    #[test]
    fn test_args_parse_invalid_delay() {
        let result = Args::try_parse_from(["mdscreensnap", "--delay", "abc"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_parse_missing_name_value() {
        let result = Args::try_parse_from(["mdscreensnap", "--name"]);
        assert!(result.is_err());
    }

    // ==================== Output Path Building Tests ====================

    #[test]
    fn test_build_output_path_no_suffix() {
        let dir = PathBuf::from("/tmp");
        let path = build_output_path(&dir, &None);
        assert!(path.starts_with("/tmp"));
        assert!(path.to_string_lossy().ends_with(".png"));
    }

    #[test]
    fn test_build_output_path_with_suffix() {
        let dir = PathBuf::from("/tmp");
        let path = build_output_path(&dir, &Some("test".to_string()));
        assert!(path.starts_with("/tmp"));
        assert!(path.to_string_lossy().contains("_test.png"));
    }

    // ==================== Capture Method Tests ====================

    #[test]
    fn test_capture_method_display_windows() {
        assert_eq!(format!("{}", CaptureMethod::Windows), "Windows (GDI)");
    }

    #[test]
    fn test_capture_method_display_wsl() {
        assert_eq!(format!("{}", CaptureMethod::Wsl), "WSL (PowerShell)");
    }

    #[test]
    fn test_capture_method_display_unsupported() {
        assert_eq!(format!("{}", CaptureMethod::Unsupported), "Unsupported");
    }

    #[test]
    fn test_capture_method_equality() {
        assert_eq!(CaptureMethod::Windows, CaptureMethod::Windows);
        assert_ne!(CaptureMethod::Windows, CaptureMethod::Wsl);
    }

    #[test]
    fn test_capture_method_clone() {
        let method = CaptureMethod::Wsl;
        let cloned = method.clone();
        assert_eq!(method, cloned);
    }

    // ==================== PowerShell Script Generation Tests ====================

    #[test]
    fn test_generate_powershell_script_basic() {
        let script = generate_powershell_script("C:\\Users\\test\\screenshot.png");
        assert!(script.contains("System.Windows.Forms"));
        assert!(script.contains("System.Drawing"));
        assert!(script.contains("CopyFromScreen"));
        assert!(script.contains("C:\\\\Users\\\\test\\\\screenshot.png"));
    }

    #[test]
    fn test_generate_powershell_script_escapes_backslashes() {
        let script = generate_powershell_script("C:\\path\\to\\file.png");
        assert!(script.contains("C:\\\\path\\\\to\\\\file.png"));
    }

    #[test]
    fn test_generate_powershell_script_escapes_quotes() {
        let script = generate_powershell_script("C:\\path\\with'quote.png");
        assert!(script.contains("with''quote"));
    }

    #[test]
    fn test_generate_powershell_script_contains_cleanup() {
        let script = generate_powershell_script("test.png");
        assert!(script.contains("Dispose()"));
    }

    // ==================== Temp Directory Tests ====================

    #[test]
    fn test_get_temp_dir_returns_valid_path() {
        let temp_dir = get_temp_dir();
        // Should return a path (may or may not exist depending on platform)
        assert!(!temp_dir.as_os_str().is_empty());
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_dry_run_creates_no_file() {
        let temp_dir = tempdir().unwrap();
        let args = Args {
            name: Some("test".to_string()),
            output: Some(temp_dir.path().to_path_buf()),
            delay: 0,
            open: false,
            dry_run: true,
        };

        let result = run(args);
        assert!(result.is_ok());

        // Check no files were created (except possibly the directory)
        let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_output_directory_created() {
        let temp_dir = tempdir().unwrap();
        let new_dir = temp_dir.path().join("screenshots");

        let args = Args {
            name: None,
            output: Some(new_dir.clone()),
            delay: 0,
            open: false,
            dry_run: true,
        };

        let result = run(args);
        assert!(result.is_ok());
        assert!(new_dir.exists());
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_filename_with_very_long_suffix() {
        let long_suffix = "a".repeat(200);
        let filename = generate_filename_with_datetime(
            &Some(long_suffix.clone()),
            "2025.12.03_143052".to_string(),
        );
        assert!(filename.contains(&long_suffix));
        assert!(filename.ends_with(".png"));
    }

    #[test]
    fn test_filename_with_whitespace_suffix() {
        let filename = generate_filename_with_datetime(
            &Some("  spaces  ".to_string()),
            "2025.12.03_143052".to_string(),
        );
        assert_eq!(filename, "2025.12.03_143052_  spaces  .png");
    }

    #[test]
    fn test_sanitize_preserves_dashes_and_underscores() {
        assert_eq!(sanitize_filename("my-file_name"), "my-file_name");
    }

    #[test]
    fn test_sanitize_preserves_dots() {
        assert_eq!(sanitize_filename("file.backup.old"), "file.backup.old");
    }
}
