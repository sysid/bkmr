// bkmr/src/infrastructure/clipboard.rs
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::services::clipboard::ClipboardService;
#[cfg(target_os = "linux")]
use tracing::debug;
use tracing::instrument;

/// Linux Clipboard Implementation Strategy:
///
/// On Linux, clipboard data is not stored in a central buffer but is "owned" by a process.
/// If bkmr exits immediately, the clipboard content is often lost because the owner is gone.
///
/// While `arboard` works well on macOS, it struggles on Linux CLI tools because:
/// 1. It requires the process to stay alive or successfully hand over data to a manager (which often times out).
/// 2. Wayland (GNOME) security policies often block the direct protocol `arboard` uses.
///
/// SOLUTION: We delegate to `wl-copy` (Wayland) or `xclip` (X11). These system utilities
/// are designed to fork into the background and manage the "ownership" lifecycle
/// correctly, ensuring the copied URL persists even after `bkmr` terminates.

#[cfg(not(target_os = "linux"))]
use arboard::Clipboard;

#[derive(Debug)]
pub struct ClipboardServiceImpl;

impl Default for ClipboardServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

impl ClipboardService for ClipboardServiceImpl {
    #[instrument(level = "trace")]
    fn copy_to_clipboard(&self, text: &str) -> DomainResult<()> {
        let clean_text = text.trim_end_matches('\n');

        #[cfg(target_os = "linux")]
        {
            self.copy_to_clipboard_linux(clean_text)
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.copy_to_clipboard_arboard(clean_text)
        }
    }
}

impl ClipboardServiceImpl {
    #[cfg(not(target_os = "linux"))]
    fn copy_to_clipboard_arboard(&self, text: &str) -> DomainResult<()> {
        match Clipboard::new() {
            Ok(mut clipboard) => clipboard.set_text(text).map_err(|e| {
                DomainError::Other(format!("Failed to set clipboard text: {}", e))
            }),
            Err(e) => Err(DomainError::Other(format!(
                "Failed to initialize clipboard: {}",
                e
            ))),
        }
    }

    #[cfg(target_os = "linux")]
    #[instrument(skip_all, level = "debug")]
    fn copy_to_clipboard_linux(&self, text: &str) -> DomainResult<()> {
        // Detect display server
        let wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

        if wayland {
            debug!("Wayland detected, using wl-copy");
            self.copy_with_wl_copy(text)
        } else {
            debug!("X11 detected, using X11 clipboard tools");
            self.copy_with_x11(text)
        }
    }

    #[cfg(target_os = "linux")]
    #[instrument(skip_all, level = "debug")]
    fn copy_with_wl_copy(&self, text: &str) -> DomainResult<()> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        debug!("Attempting clipboard copy with wl-copy");

        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| {
                DomainError::Other(
                    "wl-copy not found. Install wl-clipboard package for clipboard support on Wayland."
                        .to_string(),
                )
            })?;

        let mut stdin = child.stdin.take().ok_or_else(|| {
            DomainError::Other("Failed to open stdin pipe for wl-copy".to_string())
        })?;
        stdin.write_all(text.as_bytes()).map_err(|e| {
            DomainError::Other(format!("Failed to write to wl-copy: {}", e))
        })?;
        drop(stdin); // Explicitly close to signal EOF

        let status = child
            .wait()
            .map_err(|e| DomainError::Other(format!("Failed to wait for wl-copy: {}", e)))?;

        if status.success() {
            debug!("Successfully copied {} bytes to clipboard", text.len());
            Ok(())
        } else {
            Err(DomainError::Other(format!(
                "wl-copy failed with status: {}",
                status
            )))
        }
    }

    /// Copies text to clipboard using X11 tools (xclip → xsel fallback chain)
    #[cfg(target_os = "linux")]
    #[instrument(skip_all, level = "debug")]
    fn copy_with_x11(&self, text: &str) -> DomainResult<()> {
        debug!("Attempting clipboard copy with xclip");

        match Self::try_xclip(text) {
            Ok(()) => {
                debug!("Successfully copied {} bytes to clipboard", text.len());
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("xclip not found, falling back to xsel");
                match Self::try_xsel(text) {
                    Ok(()) => {
                        debug!("Successfully copied {} bytes to clipboard", text.len());
                        Ok(())
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(DomainError::Other(
                        "No X11 clipboard tool found. Install xclip or xsel.".to_string(),
                    )),
                    Err(e) => Err(DomainError::Other(format!("xsel failed: {}", e))),
                }
            }
            Err(e) => Err(DomainError::Other(format!("xclip failed: {}", e))),
        }
    }

    /// Try to copy using xclip. Returns io::Result to allow distinguishing NotFound.
    #[cfg(target_os = "linux")]
    fn try_xclip(text: &str) -> std::io::Result<()> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to open stdin pipe for xclip")
        })?;
        stdin.write_all(text.as_bytes())?;
        drop(stdin); // Explicitly close to signal EOF

        let status = child.wait()?;

        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("xclip exited with status: {}", status),
            ))
        }
    }

    /// Try to copy using xsel. Returns io::Result to allow distinguishing NotFound.
    #[cfg(target_os = "linux")]
    fn try_xsel(text: &str) -> std::io::Result<()> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        debug!("Attempting clipboard copy with xsel");

        let mut child = Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to open stdin pipe for xsel")
        })?;
        stdin.write_all(text.as_bytes())?;
        drop(stdin); // Explicitly close to signal EOF

        let status = child.wait()?;

        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("xsel exited with status: {}", status),
            ))
        }
    }
}

/// Dummy clipboard service for testing that doesn't interact with system clipboard
#[derive(Debug)]
pub struct DummyClipboardService;

impl DummyClipboardService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyClipboardService {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardService for DummyClipboardService {
    #[instrument(level = "trace")]
    fn copy_to_clipboard(&self, _text: &str) -> DomainResult<()> {
        // For testing, we just pretend to copy to clipboard
        Ok(())
    }
}
