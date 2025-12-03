//! mdscreensnap - A simple command-line screenshot tool for Windows and WSL

use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::Parser;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A simple command-line screenshot tool for Windows and WSL
#[derive(Parser, Debug)]
#[command(name = "mdscreensnap")]
#[command(author = "0x4D44")]
#[command(version)]
#[command(about = "Capture screenshots and save them to the temporary directory", long_about = None)]
struct Args {
    /// Custom filename suffix (optional, will be appended after the date)
    #[arg(short, long)]
    name: Option<String>,

    /// Output directory (defaults to system temp directory)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Delay in seconds before taking the screenshot
    #[arg(short, long, default_value = "0")]
    delay: u64,

    /// Open the screenshot after saving
    #[arg(long)]
    open: bool,

    /// Print the output path without capturing (dry run)
    #[arg(long)]
    dry_run: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

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
fn get_temp_dir() -> PathBuf {
    if is_wsl() {
        // On WSL, use Windows temp directory for better compatibility
        get_windows_temp_from_wsl().unwrap_or_else(|_| env::temp_dir())
    } else {
        env::temp_dir()
    }
}

/// Check if running under WSL
fn is_wsl() -> bool {
    if cfg!(windows) {
        return false;
    }

    // Check for WSL-specific indicators
    if let Ok(content) = fs::read_to_string("/proc/version") {
        if content.to_lowercase().contains("microsoft") || content.to_lowercase().contains("wsl") {
            return true;
        }
    }

    // Check for WSL interop
    std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
}

/// Get Windows temp directory from WSL
fn get_windows_temp_from_wsl() -> Result<PathBuf> {
    let output = Command::new("cmd.exe")
        .args(["/C", "echo %TEMP%"])
        .output()
        .context("Failed to get Windows TEMP directory")?;

    let windows_temp = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    // Convert Windows path to WSL path
    let output = Command::new("wslpath")
        .arg("-u")
        .arg(&windows_temp)
        .output()
        .context("Failed to convert Windows path to WSL path")?;

    let wsl_path = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    Ok(PathBuf::from(wsl_path))
}

/// Generate a filename with YYYY.MM.DD prefix
fn generate_filename(suffix: &Option<String>) -> String {
    let now = Local::now();
    let date_prefix = now.format("%Y.%m.%d_%H%M%S").to_string();

    match suffix {
        Some(s) => format!("{}_{}.png", date_prefix, sanitize_filename(s)),
        None => format!("{}.png", date_prefix),
    }
}

/// Sanitize a filename by removing or replacing invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Capture a screenshot and save it to the specified path
fn capture_screenshot(output_path: &PathBuf) -> Result<()> {
    if cfg!(windows) {
        capture_screenshot_windows(output_path)
    } else if is_wsl() {
        capture_screenshot_wsl(output_path)
    } else {
        bail!("Screenshot capture is only supported on Windows and WSL")
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

/// Capture screenshot from WSL using PowerShell
fn capture_screenshot_wsl(output_path: &PathBuf) -> Result<()> {
    // Convert WSL path to Windows path for PowerShell
    let windows_path = wsl_to_windows_path(output_path)?;

    // PowerShell script to capture the screen
    let ps_script = format!(
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
        windows_path.replace("\\", "\\\\").replace("'", "''")
    );

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
fn wsl_to_windows_path(path: &PathBuf) -> Result<String> {
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
fn open_file(path: &PathBuf) -> Result<()> {
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
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello/world"), "hello_world");
        assert_eq!(sanitize_filename("test:file"), "test_file");
        assert_eq!(sanitize_filename("normal"), "normal");
    }
}
