use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Abstraction for system side-effects
pub trait Runtime {
    /// Sleep for a duration
    fn sleep(&self, duration: Duration);

    /// Get the system temporary directory
    fn temp_dir(&self) -> PathBuf;

    /// Create a directory and all its parents
    fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Remove a file
    fn remove_file(&self, path: &Path) -> Result<()>;

    /// Read a file to string
    fn read_to_string(&self, path: &Path) -> Result<String>;

    /// Check if a path exists
    fn path_exists(&self, path: &Path) -> bool;

    /// Execute a command and return its output
    fn exec_command(&self, command: &str, args: &[&str]) -> Result<Vec<u8>>;

    /// Spawn a command (fire and forget / open)
    fn spawn_command(&self, command: &str, args: &[&str]) -> Result<()>;

    /// Get the current OS platform (windows, linux, etc) - useful for mocking
    fn is_windows(&self) -> bool;
}

/// Production implementation of Runtime
pub struct RealRuntime;

impl Runtime for RealRuntime {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn temp_dir(&self) -> PathBuf {
        std::env::temp_dir()
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).with_context(|| format!("Failed to create directory: {:?}", path))
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        fs::remove_file(path).with_context(|| format!("Failed to remove file: {:?}", path))
    }

    fn read_to_string(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {:?}", path))
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn exec_command(&self, command: &str, args: &[&str]) -> Result<Vec<u8>> {
        let output = Command::new(command)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute command: {} {:?}", command, args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Command failed: {} {:?}\nError: {}", command, args, stderr);
        }

        Ok(output.stdout)
    }

    fn spawn_command(&self, command: &str, args: &[&str]) -> Result<()> {
        Command::new(command)
            .args(args)
            .spawn()
            .with_context(|| format!("Failed to spawn command: {} {:?}", command, args))?;
        Ok(())
    }

    fn is_windows(&self) -> bool {
        cfg!(windows)
    }
}
