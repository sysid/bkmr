// bkmr/src/infrastructure/clipboard.rs
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::services::clipboard::ClipboardService;
use arboard::Clipboard;
use tracing::instrument;
// #[instrument(level = "debug")]
// pub fn copy_to_clipboard(text: &str) -> DomainResult<()> {
//     let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
//     let clean_text = text.trim_end_matches('\n');
//     clipboard
//         .set_text(clean_text)
//         .context("Failed to set clipboard text")?;
//     Ok(())
// }

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
        match Clipboard::new() {
            Ok(mut clipboard) => {
                let clean_text = text.trim_end_matches('\n');
                match clipboard.set_text(clean_text) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(DomainError::Other(format!(
                        "Failed to set clipboard text: {}",
                        e
                    ))),
                }
            }
            Err(e) => Err(DomainError::Other(format!(
                "Failed to initialize clipboard: {}",
                e
            ))),
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
