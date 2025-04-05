// src/domain/services/clipboard_service.rs
use crate::domain::error::DomainResult;

pub trait ClipboardService {
    fn copy_to_clipboard(&self, text: &str) -> DomainResult<()>;
}
