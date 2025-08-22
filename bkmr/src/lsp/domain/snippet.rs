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