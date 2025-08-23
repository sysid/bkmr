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

/// Compatibility type for existing BkmrSnippet usage
/// This maintains backwards compatibility with existing JSON deserialization
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BkmrSnippet {
    pub id: i32,
    pub title: String,
    /// Note: In the bkmr CLI output, "url" field contains the actual snippet content
    pub url: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub access_count: i32,
}

impl From<BkmrSnippet> for Snippet {
    fn from(bkmr_snippet: BkmrSnippet) -> Self {
        Self {
            id: bkmr_snippet.id,
            title: bkmr_snippet.title,
            content: bkmr_snippet.url, // url field contains the content
            description: bkmr_snippet.description,
            tags: bkmr_snippet.tags,
            access_count: bkmr_snippet.access_count,
        }
    }
}

impl From<Snippet> for BkmrSnippet {
    fn from(snippet: Snippet) -> Self {
        Self {
            id: snippet.id,
            title: snippet.title,
            url: snippet.content, // content goes to url field for compatibility
            description: snippet.description,
            tags: snippet.tags,
            access_count: snippet.access_count,
        }
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
    fn given_bkmr_snippet_when_converting_to_domain_then_maps_correctly() {
        // Arrange
        let bkmr_snippet = BkmrSnippet {
            id: 42,
            title: "Test Snippet".to_string(),
            url: "snippet content".to_string(), // url contains the actual content
            description: "test desc".to_string(),
            tags: vec!["rust".to_string(), "_snip_".to_string()],
            access_count: 5,
        };

        // Act
        let domain_snippet: Snippet = bkmr_snippet.into();

        // Assert
        assert_eq!(domain_snippet.id, 42);
        assert_eq!(domain_snippet.title, "Test Snippet");
        assert_eq!(domain_snippet.content, "snippet content");
        assert_eq!(domain_snippet.description, "test desc");
        assert_eq!(domain_snippet.tags, vec!["rust", "_snip_"]);
        assert_eq!(domain_snippet.access_count, 5);
    }

    #[test]
    fn given_domain_snippet_when_converting_to_bkmr_then_maps_correctly() {
        // Arrange
        let domain_snippet = Snippet::new(
            42,
            "Test Snippet".to_string(),
            "snippet content".to_string(),
            "test desc".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        // Act
        let bkmr_snippet: BkmrSnippet = domain_snippet.into();

        // Assert
        assert_eq!(bkmr_snippet.id, 42);
        assert_eq!(bkmr_snippet.title, "Test Snippet");
        assert_eq!(bkmr_snippet.url, "snippet content"); // content goes to url field
        assert_eq!(bkmr_snippet.description, "test desc");
        assert_eq!(bkmr_snippet.tags, vec!["rust", "_snip_"]);
        assert_eq!(bkmr_snippet.access_count, 0); // default for new snippet
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
