// bkmr/src/domain/bookmark.rs
use crate::domain::embedding::{serialize_embedding, Embedder};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use crate::util::helper::calc_content_hash;
use chrono::{DateTime, Utc};
use derive_builder::Builder;
use std::collections::HashSet;
use std::fmt;

/// Represents a bookmark domain entity
#[derive(Builder, Clone, PartialEq)]
#[builder(setter(into))]
pub struct Bookmark {
    pub id: Option<i32>,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: HashSet<Tag>,
    pub access_count: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
    #[builder(default = "false")]
    pub embeddable: bool,
}

/// Methods for the Bookmark entity
///
/// new: automatic embeddings
/// from_storage: Converts from storage format
/// Builder: no automatic embedding generation
impl Bookmark {
    pub fn new<S: AsRef<str>>(
        url: S,
        title: S,
        description: S,
        tags: HashSet<Tag>,
        embedder: &dyn Embedder,
    ) -> DomainResult<Self> {
        let url_str = url.as_ref();
        let now = Utc::now();

        // Create bookmark instance first to use get_content_for_embedding
        let mut bookmark = Self {
            id: None,
            url: url_str.to_string(),
            title: title.as_ref().to_string(),
            description: description.as_ref().to_string(),
            tags,
            access_count: 0,
            created_at: Some(now),
            updated_at: now,
            embedding: None,
            content_hash: None,
            embeddable: false, // Default to false
        };

        // Get content for embedding using the structured method
        let content = bookmark.get_content_for_embedding();

        let embedding_result = embedder
            .embed(&content)?
            .map(serialize_embedding)
            .transpose()?;

        // Only set content_hash if an embedding is created
        if embedding_result.is_some() {
            bookmark.embedding = embedding_result;
            bookmark.content_hash = Some(calc_content_hash(&content));
        }

        Ok(bookmark)
    }

    //noinspection RsExternalLinter
    pub fn from_storage(
        id: i32,
        url: String,
        title: String,
        description: String,
        tag_string: String,
        access_count: i32,
        created_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
        embedding: Option<Vec<u8>>,
        content_hash: Option<Vec<u8>>,
        embeddable: bool, // New parameter
    ) -> DomainResult<Self> {
        let tags = Tag::parse_tags(tag_string)?;

        Ok(Self {
            id: Some(id),
            url,
            title,
            description,
            tags,
            access_count,
            created_at,
            updated_at,
            embedding,
            content_hash,
            embeddable,
        })
    }

    // Add a setter for embeddable flag
    pub fn set_embeddable(&mut self, embeddable: bool) {
        self.embeddable = embeddable;
        self.updated_at = Utc::now();
    }
    /// Add a tag to the bookmark
    pub fn add_tag(&mut self, tag: Tag) -> DomainResult<()> {
        self.tags.insert(tag);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove a tag from the bookmark
    pub fn remove_tag(&mut self, tag: &Tag) -> DomainResult<()> {
        if !self.tags.remove(tag) {
            return Err(DomainError::TagOperationFailed(format!(
                "Tag '{}' not found on bookmark",
                tag
            )));
        }

        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set all tags at once (replacing existing tags)
    pub fn set_tags(&mut self, tags: HashSet<Tag>) -> DomainResult<()> {
        self.tags = tags;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Record access to the bookmark
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.updated_at = Utc::now();
    }

    /// Update bookmark information
    pub fn update(&mut self, title: String, description: String) {
        self.title = title;
        self.description = description;
        self.updated_at = Utc::now();
    }

    /// Get formatted tag string in the format ",tag1,tag2,"
    pub fn formatted_tags(&self) -> String {
        Tag::format_tags(&self.tags)
    }

    /// Get the content for embedding generation
    /// url is too noisy, so we don't include it
    pub fn get_content_for_embedding(&self) -> String {
        let visible_tags = self.get_visible_tags();

        let tags_str = Tag::format_tags(&visible_tags);
        // let normalized_url = self.url.replace('\n', " ").replace('\r', "");
        format!(
            "{}{} -- {}{}",
            tags_str, self.title, self.description, tags_str
        )
    }

    fn get_visible_tags(&self) -> HashSet<Tag> {
        // Filter out system tags (starting or ending with underscore)
        let visible_tags: HashSet<_> = self
            .tags
            .iter()
            .filter(|tag| !tag.value().starts_with('_') && !tag.value().ends_with('_'))
            .cloned()
            .collect();
        visible_tags
    }

    /// Check if the bookmark matches all given tags
    pub fn matches_all_tags(&self, tags: &HashSet<Tag>) -> bool {
        Tag::contains_all(&self.tags, tags)
    }

    /// Check if the bookmark matches any of the given tags
    pub fn matches_any_tag(&self, tags: &HashSet<Tag>) -> bool {
        Tag::contains_any(&self.tags, tags)
    }

    /// Check if the bookmark has exactly the given tags
    pub fn matches_exact_tags(&self, tags: &HashSet<Tag>) -> bool {
        self.tags == *tags
    }

    /// Set the ID (typically used after storage)
    pub fn set_id(&mut self, id: i32) {
        self.id = Some(id);
    }

    pub fn has_interpolation(&self) -> bool {
        self.url.contains("{{") || self.url.contains("{%")
    }

    /// Get snippet content (alias for url in case of snippets)
    pub fn snippet_content(&self) -> &str {
        &self.url
    }

    /// Add system tag
    pub fn add_system_tag(&mut self, system_tag: SystemTag) -> DomainResult<()> {
        self.add_tag(system_tag.to_tag()?)
    }

    /// Remove system tag
    pub fn remove_system_tag(&mut self, system_tag: SystemTag) -> DomainResult<()> {
        self.remove_tag(&system_tag.to_tag()?)
    }

    /// Get all system tags (tags that are enclosed with underscores like "_tag_")
    pub fn get_system_tags(&self) -> HashSet<Tag> {
        self.tags
            .iter()
            .filter(|tag| {
                let value = tag.value();
                value.starts_with('_') && value.ends_with('_') && value.len() > 2
            })
            .cloned()
            .collect()
    }

    /// Get all non-system tags (regular user tags)
    pub fn get_tags(&self) -> HashSet<Tag> {
        self.tags
            .iter()
            .filter(|tag| {
                let value = tag.value();
                !(value.starts_with('_') && value.ends_with('_') && value.len() > 2)
            })
            .cloned()
            .collect()
    }

    /// Check if this bookmark has a specific system tag
    pub fn is_system_tag(&self, system_tag: SystemTag) -> bool {
        self.tags.iter().any(|tag| tag.is_system_tag_of(system_tag))
    }

    /// Check if this bookmark is a snippet
    pub fn is_snippet(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.is_system_tag_of(SystemTag::Snippet))
    }

    /// Check if this bookmark is a URI (no system tags or has URI system tag)
    pub fn is_uri(&self) -> bool {
        // If it has any other system tag, it's not a URI
        !self.is_snippet()
            && !self.is_system_tag(SystemTag::Text)
            && !self.is_system_tag(SystemTag::Shell)
            && !self.is_system_tag(SystemTag::Markdown)
            && !self.is_system_tag(SystemTag::Env)
    }

    /// Check if this bookmark is a shell script
    pub fn is_shell(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.is_system_tag_of(SystemTag::Shell))
    }

    /// Check if this bookmark is a markdown document
    pub fn is_markdown(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.is_system_tag_of(SystemTag::Markdown))
    }

    /// Check if this bookmark is an environment variables set
    pub fn is_env(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.is_system_tag_of(SystemTag::Env))
    }

    /// Get the appropriate content based on bookmark type
    pub fn get_action_content(&self) -> &str {
        if self.is_snippet() {
            self.snippet_content() // For snippets, the URL is the actual content
        } else {
            &self.url // For URIs and others, use the URL
        }
    }
}

impl fmt::Display for Bookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: {} ({})",
            self.id.map_or("New".to_string(), |id| id.to_string()),
            self.title,
            self.url,
            Tag::format_tags(&self.tags)
        )
    }
}

impl fmt::Debug for Bookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bookmark")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("tags", &self.tags)
            .field("access_count", &self.access_count)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("embedding", &self.embedding.as_ref().map(|_| "[...]"))
            .field("content_hash", &self.content_hash)
            .field("embeddable", &self.embeddable)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::util::testing::init_test_env;

    #[test]
    fn test_new_bookmark_valid() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        assert_eq!(bookmark.url, "https://example.com");
        assert_eq!(bookmark.title, "Example Site");
        assert_eq!(bookmark.description, "An example website");
        assert_eq!(bookmark.tags.len(), 1);
        assert!(bookmark.tags.contains(&Tag::new("test").unwrap()));
        assert_eq!(bookmark.access_count, 0);
    }

    #[test]
    fn test_special_urls_are_valid() {
        let _ = init_test_env();
        let tags = HashSet::new();

        // Shell command URL
        let shell_url = Bookmark::new(
            "shell::echo hello",
            "Shell Command",
            "A shell command",
            tags.clone(),
            AppState::read_global().context.embedder.as_ref(),
        );
        assert!(shell_url.is_ok());

        // File path URL
        let file_url = Bookmark::new(
            "/path/to/file.txt",
            "File Path",
            "A file path",
            tags.clone(),
            AppState::read_global().context.embedder.as_ref(),
        );
        assert!(file_url.is_ok());

        // Home directory path
        let home_url = Bookmark::new(
            "~/documents/file.txt",
            "Home Path",
            "A path in home directory",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        );
        assert!(home_url.is_ok());
    }

    #[test]
    fn test_add_remove_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("initial").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Add a tag
        bookmark.add_tag(Tag::new("added").unwrap()).unwrap();
        assert_eq!(bookmark.tags.len(), 2);
        assert!(bookmark.tags.contains(&Tag::new("added").unwrap()));

        // Remove a tag
        bookmark.remove_tag(&Tag::new("initial").unwrap()).unwrap();
        assert_eq!(bookmark.tags.len(), 1);
        assert!(!bookmark.tags.contains(&Tag::new("initial").unwrap()));

        // Try to remove a non-existent tag
        let result = bookmark.remove_tag(&Tag::new("nonexistent").unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_set_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("initial").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Set completely new tags
        let mut new_tags = HashSet::new();
        new_tags.insert(Tag::new("new1").unwrap());
        new_tags.insert(Tag::new("new2").unwrap());

        bookmark.set_tags(new_tags.clone()).unwrap();
        assert_eq!(bookmark.tags, new_tags);
        assert_eq!(bookmark.tags.len(), 2);
    }

    #[test]
    fn test_record_access() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        assert_eq!(bookmark.access_count, 0);

        bookmark.record_access();
        assert_eq!(bookmark.access_count, 1);

        bookmark.record_access();
        assert_eq!(bookmark.access_count, 2);
    }

    #[test]
    fn test_formatted_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("tag1").unwrap());
        tags.insert(Tag::new("tag2").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let formatted = bookmark.formatted_tags();
        assert!(formatted == ",tag1,tag2," || formatted == ",tag2,tag1,");
    }

    #[test]
    fn test_get_content_for_embedding() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("visible").unwrap());
        tags.insert(Tag::new("_system").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        let content = bookmark.get_content_for_embedding();
        assert!(content.contains("visible"));
        assert!(!content.contains("_system"));
        assert!(content.contains("Example Site"));
        assert!(content.contains("An example website"));
    }

    #[test]
    fn test_tag_matching() {
        let _ = init_test_env();
        let mut bookmark_tags = HashSet::new();
        bookmark_tags.insert(Tag::new("tag1").unwrap());
        bookmark_tags.insert(Tag::new("tag2").unwrap());
        bookmark_tags.insert(Tag::new("tag3").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            bookmark_tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test matches_all_tags
        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag2").unwrap());

        assert!(bookmark.matches_all_tags(&query_tags));

        query_tags.insert(Tag::new("tag4").unwrap());
        assert!(!bookmark.matches_all_tags(&query_tags));

        // Test matches_any_tag
        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag4").unwrap());

        assert!(bookmark.matches_any_tag(&query_tags));

        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag4").unwrap());
        query_tags.insert(Tag::new("tag5").unwrap());

        assert!(!bookmark.matches_any_tag(&query_tags));

        // Test matches_exact_tags
        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag2").unwrap());
        query_tags.insert(Tag::new("tag3").unwrap());

        assert!(bookmark.matches_exact_tags(&query_tags));

        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag2").unwrap());

        assert!(!bookmark.matches_exact_tags(&query_tags));
    }

    #[test]
    fn test_get_system_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();

        // Add regular tags
        tags.insert(Tag::new("regular1").unwrap());
        tags.insert(Tag::new("regular2").unwrap());
        tags.insert(Tag::new("_partial").unwrap()); // Not a system tag
        tags.insert(Tag::new("partial_").unwrap()); // Not a system tag

        // Add system tags (enclosed with underscores)
        tags.insert(Tag::new("_system1_").unwrap());
        tags.insert(Tag::new("_system2_").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test get_system_tags
        let system_tags = bookmark.get_system_tags();
        assert_eq!(system_tags.len(), 2);
        assert!(system_tags.contains(&Tag::new("_system1_").unwrap()));
        assert!(system_tags.contains(&Tag::new("_system2_").unwrap()));
        assert!(!system_tags.contains(&Tag::new("_partial").unwrap()));
        assert!(!system_tags.contains(&Tag::new("partial_").unwrap()));
    }

    #[test]
    fn test_get_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();

        // Add regular tags
        tags.insert(Tag::new("regular1").unwrap());
        tags.insert(Tag::new("regular2").unwrap());
        tags.insert(Tag::new("_partial").unwrap()); // This is a regular tag
        tags.insert(Tag::new("partial_").unwrap()); // This is a regular tag

        // Add system tags
        tags.insert(Tag::new("_system1_").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test get_tags
        let regular_tags = bookmark.get_tags();
        assert_eq!(regular_tags.len(), 4);
        assert!(regular_tags.contains(&Tag::new("regular1").unwrap()));
        assert!(regular_tags.contains(&Tag::new("regular2").unwrap()));
        assert!(regular_tags.contains(&Tag::new("_partial").unwrap()));
        assert!(regular_tags.contains(&Tag::new("partial_").unwrap()));
        assert!(!regular_tags.contains(&Tag::new("_system1_").unwrap()));
    }

    #[test]
    fn test_get_tags_with_only_system_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();

        // Add only system tags
        tags.insert(Tag::new("_system1_").unwrap());
        tags.insert(Tag::new("_system2_").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test get_tags returns empty set when only system tags exist
        let regular_tags = bookmark.get_tags();
        assert_eq!(regular_tags.len(), 0);

        // Test get_system_tags returns all system tags
        let system_tags = bookmark.get_system_tags();
        assert_eq!(system_tags.len(), 2);
    }

    #[test]
    fn test_embeddable_flag() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Default should be false
        assert!(!bookmark.embeddable);

        // Set to true
        bookmark.set_embeddable(true);
        assert!(bookmark.embeddable);

        // Set back to false
        bookmark.set_embeddable(false);
        assert!(!bookmark.embeddable);
    }
    #[test]
    fn test_is_system_tag() {
        let _ = init_test_env();

        // Create a bookmark with the Text system tag
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_imported_").unwrap()); // Text system tag

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test is_system_tag
        assert!(bookmark.is_system_tag(SystemTag::Text));
        assert!(!bookmark.is_system_tag(SystemTag::Snippet));
    }

    #[test]
    fn test_is_uri() {
        let _ = init_test_env();

        // Create a regular URI bookmark
        let tags_uri = HashSet::new();
        let bookmark_uri = Bookmark::new(
            "https://example.com",
            "Example Site",
            "A website with no system tags",
            tags_uri,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Create a snippet bookmark
        let mut tags_snippet = HashSet::new();
        tags_snippet.insert(Tag::new("_snip_").unwrap());
        let bookmark_snippet = Bookmark::new(
            "print('Hello world')",
            "Python Snippet",
            "A Python code snippet",
            tags_snippet,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test is_uri
        assert!(bookmark_uri.is_uri());
        assert!(!bookmark_snippet.is_uri());
    }

    #[test]
    fn test_get_action_content() {
        let _ = init_test_env();

        // Create a URI bookmark
        let tags_uri = HashSet::new();
        let bookmark_uri = Bookmark::new(
            "https://example.com",
            "Example Site",
            "A website",
            tags_uri,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Create a snippet bookmark
        let mut tags_snippet = HashSet::new();
        tags_snippet.insert(Tag::new("_snip_").unwrap());
        let snippet_content = "print('Hello world')";
        let bookmark_snippet = Bookmark::new(
            snippet_content,
            "Python Snippet",
            "A Python code snippet",
            tags_snippet,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test get_action_content
        assert_eq!(bookmark_uri.get_action_content(), "https://example.com");
        assert_eq!(bookmark_snippet.get_action_content(), snippet_content);
    }
}
