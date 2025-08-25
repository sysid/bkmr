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

/// Service for managing document state and extracting completion queries
#[derive(Debug)]
pub struct DocumentService {
    /// Cache of document contents
    document_cache: Arc<RwLock<HashMap<String, String>>>,
    /// Cache of document language IDs
    language_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl DocumentService {
    pub fn new() -> Self {
        Self {
            document_cache: Arc::new(RwLock::new(HashMap::new())),
            language_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new document
    #[instrument(skip(self, content))]
    pub async fn open_document(
        &self,
        uri: String,
        language_id: String,
        content: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Opening document: {} (language: {})", uri, language_id);

        {
            let mut cache = self.document_cache.write().await;
            cache.insert(uri.clone(), content);
        }

        {
            let mut lang_cache = self.language_cache.write().await;
            lang_cache.insert(uri, language_id);
        }

        Ok(())
    }

    /// Update document content
    #[instrument(skip(self, content))]
    pub async fn update_document(
        &self,
        uri: String,
        content: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Updating document: {}", uri);

        let mut cache = self.document_cache.write().await;
        cache.insert(uri, content);

        Ok(())
    }

    /// Close a document and remove from cache
    #[instrument(skip(self))]
    pub async fn close_document(
        &self,
        uri: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Closing document: {}", uri);

        {
            let mut cache = self.document_cache.write().await;
            cache.remove(&uri);
        }

        {
            let mut lang_cache = self.language_cache.write().await;
            lang_cache.remove(&uri);
        }

        Ok(())
    }

    /// Get the language ID for a document
    pub async fn get_language_id(&self, uri: &str) -> Option<String> {
        let cache = self.language_cache.read().await;
        cache.get(uri).cloned()
    }

    /// Extract completion context from document position
    #[instrument(skip(self))]
    pub async fn extract_completion_context(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<CompletionContext, Box<dyn std::error::Error + Send + Sync>> {
        let language_id = self.get_language_id(uri.as_ref()).await;
        let mut context = CompletionContext::new(uri.clone(), position, language_id);

        if let Some(query) = self.extract_snippet_query(uri, position).await? {
            context = context.with_query(query);
        }

        Ok(context)
    }

    /// Extract word backwards from cursor position and return both query and range
    #[instrument(skip(self))]
    async fn extract_snippet_query(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Option<CompletionQuery>, Box<dyn std::error::Error + Send + Sync>> {
        let cache = self.document_cache.read().await;
        let content = cache
            .get(&uri.to_string())
            .ok_or("Document not found in cache")?;

        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return Ok(None);
        }

        let line = lines[position.line as usize];
        let char_pos = position.character as usize;

        if char_pos > line.len() {
            return Ok(None);
        }

        let before_cursor = &line[..char_pos];
        debug!(
            "Extracting from line: '{}', char_pos: {}, before_cursor: '{}'",
            line, char_pos, before_cursor
        );

        // Extract word backwards from cursor - find where the word starts
        let word_start = before_cursor
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-')
            .last()
            .map(|(i, _)| i)
            .unwrap_or(char_pos);

        debug!("Word boundaries: start={}, end={}", word_start, char_pos);

        if word_start < char_pos {
            let word = &before_cursor[word_start..];
            if !word.is_empty() && word.chars().any(|c| c.is_alphanumeric()) {
                debug!("Extracted word: '{}' from position {}", word, char_pos);

                // Create range for the word to be replaced
                let range = Range {
                    start: Position {
                        line: position.line,
                        character: word_start as u32,
                    },
                    end: Position {
                        line: position.line,
                        character: char_pos as u32,
                    },
                };

                return Ok(Some(CompletionQuery::new(word.to_string(), range)));
            }
        }

        debug!("No valid word found at position {}", char_pos);
        Ok(None)
    }

    /// Extract snippet query synchronously for compatibility with existing backend
    /// This is a temporary bridge method to maintain compatibility
    pub fn extract_snippet_query_sync(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<(String, Range)> {
        // Try to get read lock without waiting
        if let Ok(cache) = self.document_cache.try_read() {
            let content = cache.get(&uri.to_string())?;

            let lines: Vec<&str> = content.lines().collect();
            if position.line as usize >= lines.len() {
                return None;
            }

            let line = lines[position.line as usize];
            let char_pos = position.character as usize;

            if char_pos > line.len() {
                return None;
            }

            let before_cursor = &line[..char_pos];
            debug!(
                "Extracting from line: '{}', char_pos: {}, before_cursor: '{}'",
                line, char_pos, before_cursor
            );

            // Extract word backwards from cursor - find where the word starts
            let word_start = before_cursor
                .char_indices()
                .rev()
                .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-')
                .last()
                .map(|(i, _)| i)
                .unwrap_or(char_pos);

            debug!("Word boundaries: start={}, end={}", word_start, char_pos);

            if word_start < char_pos {
                let word = &before_cursor[word_start..];
                if !word.is_empty() && word.chars().any(|c| c.is_alphanumeric()) {
                    debug!("Extracted word: '{}' from position {}", word, char_pos);

                    // Create range for the word to be replaced
                    let range = Range {
                        start: Position {
                            line: position.line,
                            character: word_start as u32,
                        },
                        end: Position {
                            line: position.line,
                            character: char_pos as u32,
                        },
                    };

                    return Some((word.to_string(), range));
                }
            }

            debug!("No valid word found at position {}", char_pos);
            None
        } else {
            None
        }
    }

    /// Get language ID synchronously for compatibility with existing backend
    pub fn get_language_id_sync(&self, uri: &Url) -> Option<String> {
        if let Ok(cache) = self.language_cache.try_read() {
            cache.get(&uri.to_string()).cloned()
        } else {
            None
        }
    }
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
        let result = service
            .open_document(uri.clone(), language_id.clone(), content.clone())
            .await;

        // Assert
        assert!(result.is_ok());
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
            .await
            .expect("open document");

        // Act
        let result = service.extract_completion_context(&uri, position).await;

        // Assert
        assert!(result.is_ok());
        let context = result.expect("valid completion context");
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
            .await
            .expect("open document");

        // Act
        let result = service.extract_completion_context(&uri, position).await;

        // Assert
        assert!(result.is_ok());
        let context = result.expect("valid completion context");
        assert!(!context.has_query());
    }

    #[tokio::test]
    async fn given_document_when_closing_then_removes_from_cache() {
        // Arrange
        let service = DocumentService::new();
        let uri = "file:///test.rs".to_string();

        service
            .open_document(uri.clone(), "rust".to_string(), "content".to_string())
            .await
            .expect("open document");
        assert!(service.get_language_id(&uri).await.is_some());

        // Act
        let result = service.close_document(uri.clone()).await;

        // Assert
        assert!(result.is_ok());
        assert!(service.get_language_id(&uri).await.is_none());
    }
}
