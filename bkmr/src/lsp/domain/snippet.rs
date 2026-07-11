use serde::{Deserialize, Serialize};

/// Core snippet domain model representing a bkmr snippet
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Snippet {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub access_count: i32,
}

impl Snippet {
    /// Create a new snippet
    pub fn new(
        id: i32,
        title: String,
        content: String,
        description: String,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id,
            title,
            content,
            description,
            tags,
            access_count: 0,
        }
    }

    /// Check if this snippet is marked as universal
    pub fn is_universal(&self) -> bool {
        self.tags.contains(&"universal".to_string())
    }

    /// Check if this snippet is marked as a snippet
    pub fn is_snippet(&self) -> bool {
        self.tags.contains(&"_snip_".to_string())
    }

    /// Check if this snippet has a specific language tag
    pub fn has_language(&self, language: &str) -> bool {
        self.tags.contains(&language.to_string())
    }

    /// Check if this snippet is marked as plain text (no snippet formatting)
    pub fn is_plain(&self) -> bool {
        self.tags.contains(&"plain".to_string())
    }

    /// Get the snippet content (content field contains actual snippet data)
    pub fn get_content(&self) -> &str {
        &self.content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_universal_tag_when_checking_is_universal_then_returns_true() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Test".to_string(),
            "content".to_string(),
            "desc".to_string(),
            vec!["universal".to_string()],
        );

        // Act
        let is_universal = snippet.is_universal();

        // Assert
        assert!(is_universal);
    }

    #[test]
    fn given_no_universal_tag_when_checking_is_universal_then_returns_false() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Test".to_string(),
            "content".to_string(),
            "desc".to_string(),
            vec!["rust".to_string()],
        );

        // Act
        let is_universal = snippet.is_universal();

        // Assert
        assert!(!is_universal);
    }

    #[test]
    fn given_plain_tag_when_checking_is_plain_then_returns_true() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Plain Text".to_string(),
            "simple text content".to_string(),
            "Plain text snippet".to_string(),
            vec!["plain".to_string(), "_snip_".to_string()],
        );

        // Act
        let is_plain = snippet.is_plain();

        // Assert
        assert!(is_plain);
    }

    #[test]
    fn given_no_plain_tag_when_checking_is_plain_then_returns_false() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Regular Snippet".to_string(),
            "snippet with ${1:placeholder}".to_string(),
            "Regular snippet".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        // Act
        let is_plain = snippet.is_plain();

        // Assert
        assert!(!is_plain);
    }
}
