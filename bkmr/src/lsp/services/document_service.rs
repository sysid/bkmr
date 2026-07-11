//! Document service for managing LSP document state
//!
//! Handles document lifecycle (open, change, close) and provides document content access
//! for completion and other LSP operations.

use crate::lsp::domain::{CompletionContext, CompletionQuery};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{Position, Range, Url};
use tracing::{debug, instrument};

/// Per-document state tracked for open documents
#[derive(Debug)]
struct DocumentState {
    content: String,
    language_id: String,
}

/// Service for managing document state and extracting completion queries
#[derive(Debug)]
pub struct DocumentService {
    documents: Arc<RwLock<HashMap<String, DocumentState>>>,
}

impl DocumentService {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new document
    #[instrument(skip(self, content))]
    pub async fn open_document(&self, uri: String, language_id: String, content: String) {
        debug!("Opening document: {} (language: {})", uri, language_id);
        let mut docs = self.documents.write().await;
        docs.insert(
            uri,
            DocumentState {
                content,
                language_id,
            },
        );
    }

    /// Update document content
    #[instrument(skip(self, content))]
    pub async fn update_document(&self, uri: String, content: String) {
        debug!("Updating document: {}", uri);
        let mut docs = self.documents.write().await;
        match docs.get_mut(&uri) {
            Some(state) => state.content = content,
            None => {
                // didChange before didOpen — accept the content with unknown language
                docs.insert(
                    uri,
                    DocumentState {
                        content,
                        language_id: String::new(),
                    },
                );
            }
        }
    }

    /// Close a document and remove from cache
    #[instrument(skip(self))]
    pub async fn close_document(&self, uri: String) {
        debug!("Closing document: {}", uri);
        let mut docs = self.documents.write().await;
        docs.remove(&uri);
    }

    /// Get the language ID for a document
    pub async fn get_language_id(&self, uri: &str) -> Option<String> {
        let docs = self.documents.read().await;
        docs.get(uri)
            .map(|s| s.language_id.clone())
            .filter(|l| !l.is_empty())
    }

    /// Extract completion context from document position.
    ///
    /// Total: an unopened document or out-of-range position yields a context
    /// without query rather than an error, so completion degrades gracefully.
    #[instrument(skip(self))]
    pub async fn extract_completion_context(
        &self,
        uri: &Url,
        position: Position,
    ) -> CompletionContext {
        let docs = self.documents.read().await;
        let state = docs.get(&uri.to_string());

        let language_id = state
            .map(|s| s.language_id.clone())
            .filter(|l| !l.is_empty());
        let mut context = CompletionContext::new(uri.clone(), position, language_id);

        if let Some(state) = state {
            if let Some(line) = state.content.lines().nth(position.line as usize) {
                if let Some(query) = extract_query_from_line(line, position) {
                    context = context.with_query(query);
                }
            }
        }

        context
    }
}

/// Extract the word ending at the cursor and its replacement range.
///
/// `position.character` is in UTF-16 code units per the LSP spec, so it must be
/// converted to a byte offset before slicing; the returned range is UTF-16 again.
fn extract_query_from_line(line: &str, position: Position) -> Option<CompletionQuery> {
    let cursor_byte = utf16_to_byte_offset(line, position.character)?;
    let before_cursor = &line[..cursor_byte];

    // Extract word backwards from cursor - find where the word starts
    let word_start_byte = before_cursor
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-')
        .last()
        .map(|(i, _)| i)
        .unwrap_or(cursor_byte);

    if word_start_byte >= cursor_byte {
        debug!("No valid word found at position {}", position.character);
        return None;
    }

    let word = &before_cursor[word_start_byte..];
    if !word.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }

    debug!("Extracted word: '{}' at line {}", word, position.line);

    let word_start_utf16: u32 = line[..word_start_byte]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum();

    Some(CompletionQuery::new(
        word.to_string(),
        Range {
            start: Position {
                line: position.line,
                character: word_start_utf16,
            },
            end: position,
        },
    ))
}

/// Convert a UTF-16 code-unit offset into a byte offset within `line`.
///
/// Returns None if the offset is past the end of the line or falls inside a
/// surrogate pair (a position no well-behaved client sends).
fn utf16_to_byte_offset(line: &str, utf16_offset: u32) -> Option<usize> {
    let mut units: u32 = 0;
    for (byte_idx, c) in line.char_indices() {
        if units == utf16_offset {
            return Some(byte_idx);
        }
        units += c.len_utf16() as u32;
        if units > utf16_offset {
            return None;
        }
    }
    (units == utf16_offset).then_some(line.len())
}

impl Default for DocumentService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Position;

    #[tokio::test]
    async fn given_new_document_when_opening_then_stores_correctly() {
        // Arrange
        let service = DocumentService::new();
        let uri = "file:///test.rs".to_string();
        let language_id = "rust".to_string();
        let content = "fn main() {}".to_string();

        // Act
        service
            .open_document(uri.clone(), language_id.clone(), content.clone())
            .await;

        // Assert
        let stored_language = service.get_language_id(&uri).await;
        assert_eq!(stored_language, Some(language_id));
    }

    #[tokio::test]
    async fn given_document_with_word_when_extracting_query_then_finds_word() {
        // Arrange
        let service = DocumentService::new();
        let uri_str = "file:///test.rs".to_string();
        let uri = Url::parse(&uri_str).expect("parse URI");
        let content = "hello world".to_string();
        let position = Position {
            line: 0,
            character: 5,
        }; // End of "hello"

        service
            .open_document(uri_str, "rust".to_string(), content)
            .await;

        // Act
        let context = service.extract_completion_context(&uri, position).await;

        // Assert
        assert!(context.has_query());
        assert_eq!(context.get_query_text(), Some("hello"));
    }

    #[tokio::test]
    async fn given_document_without_word_when_extracting_query_then_returns_none() {
        // Arrange
        let service = DocumentService::new();
        let uri_str = "file:///test.rs".to_string();
        let uri = Url::parse(&uri_str).expect("parse URI");
        let content = "   ".to_string(); // Only whitespace
        let position = Position {
            line: 0,
            character: 2,
        };

        service
            .open_document(uri_str, "rust".to_string(), content)
            .await;

        // Act
        let context = service.extract_completion_context(&uri, position).await;

        // Assert
        assert!(!context.has_query());
    }

    #[tokio::test]
    async fn given_line_with_emoji_before_word_when_extracting_query_then_uses_utf16_positions() {
        // LSP positions are UTF-16 code units. "🚀🚀abc" is 4+3=7 UTF-16 units
        // but 8+3=11 bytes; byte-slicing at the UTF-16 offset panics mid-char.
        let service = DocumentService::new();
        let uri_str = "file:///test.rs".to_string();
        let uri = Url::parse(&uri_str).expect("parse URI");
        service
            .open_document(uri_str, "rust".to_string(), "🚀🚀abc".to_string())
            .await;

        let position = Position {
            line: 0,
            character: 7, // after "abc" in UTF-16 units
        };

        let context = service.extract_completion_context(&uri, position).await;

        assert_eq!(context.get_query_text(), Some("abc"));
        let range = context.get_replacement_range().expect("range");
        assert_eq!(range.start.character, 4); // UTF-16 offset of 'a'
        assert_eq!(range.end.character, 7);
    }

    #[tokio::test]
    async fn given_position_past_line_end_when_extracting_query_then_returns_none() {
        let service = DocumentService::new();
        let uri_str = "file:///test.rs".to_string();
        let uri = Url::parse(&uri_str).expect("parse URI");
        service
            .open_document(uri_str, "rust".to_string(), "abc".to_string())
            .await;

        let position = Position {
            line: 0,
            character: 42,
        };

        let context = service.extract_completion_context(&uri, position).await;

        assert!(!context.has_query());
    }

    #[tokio::test]
    async fn given_unopened_document_when_extracting_context_then_returns_context_without_query() {
        let service = DocumentService::new();
        let uri = Url::parse("file:///never-opened.rs").expect("parse URI");

        let context = service
            .extract_completion_context(
                &uri,
                Position {
                    line: 0,
                    character: 0,
                },
            )
            .await;

        assert!(!context.has_query());
        assert!(context.language_id.is_none());
    }

    #[tokio::test]
    async fn given_document_when_closing_then_removes_from_cache() {
        // Arrange
        let service = DocumentService::new();
        let uri = "file:///test.rs".to_string();

        service
            .open_document(uri.clone(), "rust".to_string(), "content".to_string())
            .await;
        assert!(service.get_language_id(&uri).await.is_some());

        // Act
        service.close_document(uri.clone()).await;

        // Assert
        assert!(service.get_language_id(&uri).await.is_none());
    }
}
