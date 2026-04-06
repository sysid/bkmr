// bkmr/src/domain/bookmark.rs
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use chrono::{DateTime, Utc};
use derive_builder::Builder;
use std::collections::HashSet;
use std::fmt;

/// Build the text content used for embedding generation.
/// Filters out system tags and constructs: `"{tags}{title} -- {content}{tags}"`
pub fn build_embedding_content(tags: &HashSet<Tag>, title: &str, content: &str) -> String {
    let visible_tags: HashSet<_> = tags
        .iter()
        .filter(|tag| !tag.value().starts_with('_') && !tag.value().ends_with('_'))
        .cloned()
        .collect();
    let tags_str = Tag::format_tags(&visible_tags);
    format!("{}{} -- {}{}", tags_str, title, content, tags_str)
}

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
    #[builder(default)]
    pub file_path: Option<String>,
    #[builder(default)]
    pub file_mtime: Option<i32>,
    #[builder(default)]
    pub file_hash: Option<String>,
    #[builder(default)]
    pub opener: Option<String>,
    #[builder(default)]
    pub accessed_at: Option<DateTime<Utc>>,
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
    ) -> DomainResult<Self> {
        let url_str = url.as_ref();
        let now = Utc::now();

        // Create bookmark instance first to use get_content_for_embedding
        let bookmark = Self {
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
            file_path: None,
            file_mtime: None,
            file_hash: None,
            opener: None,
            accessed_at: None,
        };

        Self::validate_single_system_tag(&bookmark.tags)?;
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
        embeddable: bool,
        file_path: Option<String>,
        file_mtime: Option<i32>,
        file_hash: Option<String>,
        opener: Option<String>,
        accessed_at: Option<DateTime<Utc>>,
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
            file_path,
            file_mtime,
            file_hash,
            opener,
            accessed_at,
        })
    }

    // Add a setter for embeddable flag
    pub fn set_embeddable(&mut self, embeddable: bool) {
        self.embeddable = embeddable;
        self.updated_at = Utc::now();
    }
    /// Validate that at most one known system tag is present in a tag set.
    fn validate_single_system_tag(tags: &HashSet<Tag>) -> DomainResult<()> {
        let system_tags: Vec<_> = tags.iter().filter(|t| t.is_known_system_tag()).collect();
        if system_tags.len() > 1 {
            let names: Vec<_> = system_tags.iter().map(|t| t.value().to_string()).collect();
            return Err(DomainError::TagOperationFailed(format!(
                "Bookmark may have at most one system tag, found: {}",
                names.join(", ")
            )));
        }
        Ok(())
    }

    /// Add a tag to the bookmark
    pub fn add_tag(&mut self, tag: Tag) -> DomainResult<()> {
        let was_new = self.tags.insert(tag.clone());
        if was_new {
            if let Err(e) = Self::validate_single_system_tag(&self.tags) {
                self.tags.remove(&tag);
                return Err(e);
            }
        }
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
        Self::validate_single_system_tag(&tags)?;
        self.tags = tags;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Record access to the bookmark (does not change updated_at)
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.accessed_at = Some(Utc::now());
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

    /// Build the text string used for embedding generation, dispatched by system tag.
    ///
    /// For content-bearing types (_snip_, _shell_, _md_, _env_, _imported_, _mem_), the
    /// actual content lives in `self.url`. For URI bookmarks (no system tag), the url is
    /// a link and the meaningful text is in `self.description`.
    ///
    /// The result is passed to `build_embedding_content()` which prepends tags and title.
    pub fn get_content_for_embedding(&self) -> String {
        let content = if self.is_snippet() {
            &self.url // code snippet
        } else if self.is_system_tag(SystemTag::Shell) {
            &self.url // shell script
        } else if self.is_system_tag(SystemTag::Markdown) {
            &self.url // markdown document
        } else if self.is_system_tag(SystemTag::Env) {
            &self.url // environment variables
        } else if self.is_system_tag(SystemTag::Text) {
            &self.url // imported text content
        } else if self.is_system_tag(SystemTag::Memory) {
            &self.url // agent memory content
        } else {
            // URI bookmarks and unknown types: url is a link, description has the content
            &self.description
        };
        build_embedding_content(&self.tags, &self.title, content)
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

    /// Check if this bookmark is an agent memory
    pub fn is_memory(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.is_system_tag_of(SystemTag::Memory))
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
            .field("accessed_at", &self.accessed_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::init_test_env;

    #[test]
    fn given_valid_bookmark_data_when_new_then_creates_bookmark() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
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
    fn given_special_urls_when_validate_then_accepts_as_valid() {
        let _ = init_test_env();
        let tags = HashSet::new();

        // Shell command URL
        let shell_url = Bookmark::new(
            "shell::echo hello",
            "Shell Command",
            "A shell command",
            tags.clone(),
        );
        assert!(shell_url.is_ok());

        // File path URL
        let file_url = Bookmark::new(
            "/path/to/file.txt",
            "File Path",
            "A file path",
            tags.clone(),
        );
        assert!(file_url.is_ok());

        // Home directory path
        let home_url = Bookmark::new(
            "~/documents/file.txt",
            "Home Path",
            "A path in home directory",
            tags,
        );
        assert!(home_url.is_ok());
    }

    #[test]
    fn given_bookmark_when_add_remove_tags_then_updates_tag_set() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("initial").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
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
    fn given_bookmark_when_set_tags_then_replaces_tag_set() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("initial").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
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
    fn given_bookmark_when_record_access_then_increments_count_and_sets_accessed_at() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
        )
        .unwrap();

        assert_eq!(bookmark.access_count, 0);
        assert!(bookmark.accessed_at.is_none());
        let updated_at_before = bookmark.updated_at;

        bookmark.record_access();
        assert_eq!(bookmark.access_count, 1);
        assert!(bookmark.accessed_at.is_some());
        assert_eq!(bookmark.updated_at, updated_at_before, "record_access must not change updated_at");

        bookmark.record_access();
        assert_eq!(bookmark.access_count, 2);
        assert_eq!(bookmark.updated_at, updated_at_before, "record_access must not change updated_at");
    }

    #[test]
    fn given_bookmark_with_tags_when_format_then_returns_formatted_string() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("tag1").unwrap());
        tags.insert(Tag::new("tag2").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
        )
        .unwrap();

        let formatted = bookmark.formatted_tags();
        assert!(formatted == ",tag1,tag2," || formatted == ",tag2,tag1,");
    }

    #[test]
    fn given_bookmark_when_get_embedding_content_then_returns_concatenated_text() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("visible").unwrap());
        tags.insert(Tag::new("_system").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
        )
        .unwrap();

        let content = bookmark.get_content_for_embedding();
        assert!(content.contains("visible"));
        assert!(!content.contains("_system"));
        assert!(content.contains("Example Site"));
        assert!(content.contains("An example website"));
    }

    #[test]
    fn given_snippet_when_get_embedding_content_then_uses_url_as_content() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("sql").unwrap());
        tags.insert(Tag::new("_snip_").unwrap());

        let bookmark = Bookmark::new(
            "SELECT * FROM users WHERE active = true", // url = the snippet code
            "User Query",                               // title
            "Finds active users",                       // description (should NOT be embedded)
            tags,
        )
        .unwrap();

        let content = bookmark.get_content_for_embedding();
        assert!(
            content.contains("SELECT * FROM users"),
            "should embed url (the snippet code)"
        );
        assert!(
            !content.contains("Finds active users"),
            "should NOT embed description"
        );
        assert!(content.contains("User Query"), "should embed title");
        assert!(content.contains("sql"), "should embed visible tags");
        assert!(!content.contains("_snip_"), "should NOT embed system tags");
    }

    #[test]
    fn given_shell_bookmark_when_get_embedding_content_then_uses_url_as_content() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("utils").unwrap());
        tags.insert(Tag::new("_shell_").unwrap());

        let bookmark = Bookmark::new(
            "#!/bin/bash\necho 'hello'", // url = the shell script
            "Greeting Script",            // title
            "",                           // description empty (typical for shell)
            tags,
        )
        .unwrap();

        let content = bookmark.get_content_for_embedding();
        assert!(
            content.contains("#!/bin/bash"),
            "should embed url (the shell script)"
        );
        assert!(
            content.contains("Greeting Script"),
            "should embed title"
        );
    }

    #[test]
    fn given_uri_bookmark_when_get_embedding_content_then_uses_description() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("rust").unwrap());

        let bookmark = Bookmark::new(
            "https://www.rust-lang.org", // url = a link (should NOT be embedded)
            "Rust Language",              // title
            "A systems programming language", // description (should be embedded)
            tags,
        )
        .unwrap();

        let content = bookmark.get_content_for_embedding();
        assert!(
            content.contains("systems programming"),
            "should embed description"
        );
        assert!(
            !content.contains("rust-lang.org"),
            "should NOT embed URL"
        );
    }

    #[test]
    fn given_memory_bookmark_when_get_embedding_content_then_uses_url_as_content() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("project").unwrap());
        tags.insert(Tag::new("_mem_").unwrap());

        let bookmark = Bookmark::new(
            "The auth service uses JWT tokens with 24h expiry", // url = memory content
            "Auth Token Policy",                                 // title
            "",                                                  // description empty
            tags,
        )
        .unwrap();

        assert!(bookmark.is_memory());
        let content = bookmark.get_content_for_embedding();
        assert!(
            content.contains("JWT tokens"),
            "should embed url (the memory content)"
        );
        assert!(
            content.contains("Auth Token Policy"),
            "should embed title"
        );
        assert!(content.contains("project"), "should embed visible tags");
        assert!(!content.contains("_mem_"), "should NOT embed system tags");
    }

    #[test]
    fn given_bookmark_with_tags_when_match_then_validates_tag_presence() {
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
    fn given_bookmark_with_mixed_tags_when_get_system_tags_then_filters_system_only() {
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
    fn given_bookmark_with_mixed_tags_when_get_tags_then_filters_user_only() {
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
    fn given_bookmark_with_only_system_tags_when_get_tags_then_returns_empty() {
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
    fn given_bookmark_when_set_embeddable_then_updates_flag() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
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
    fn given_tag_when_check_system_then_validates_system_status() {
        let _ = init_test_env();

        // Create a bookmark with the Text system tag
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_imported_").unwrap()); // Text system tag

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
        )
        .unwrap();

        // Test is_system_tag
        assert!(bookmark.is_system_tag(SystemTag::Text));
        assert!(!bookmark.is_system_tag(SystemTag::Snippet));
    }

    #[test]
    fn given_string_when_check_uri_then_validates_uri_format() {
        let _ = init_test_env();

        // Create a regular URI bookmark
        let tags_uri = HashSet::new();
        let bookmark_uri = Bookmark::new(
            "https://example.com",
            "Example Site",
            "A website with no system tags",
            tags_uri,
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
        )
        .unwrap();

        // Test is_uri
        assert!(bookmark_uri.is_uri());
        assert!(!bookmark_snippet.is_uri());
    }

    #[test]
    fn given_bookmark_when_get_action_content_then_returns_appropriate_content() {
        let _ = init_test_env();

        // Create a URI bookmark
        let tags_uri = HashSet::new();
        let bookmark_uri = Bookmark::new(
            "https://example.com",
            "Example Site",
            "A website",
            tags_uri,
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
        )
        .unwrap();

        // Test get_action_content
        assert_eq!(bookmark_uri.get_action_content(), "https://example.com");
        assert_eq!(bookmark_snippet.get_action_content(), snippet_content);
    }

    #[test]
    fn given_multiple_known_system_tags_when_new_then_returns_error() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_snip_").unwrap());
        tags.insert(Tag::new("_shell_").unwrap());

        let result = Bookmark::new("content", "title", "desc", tags);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("at most one system tag"), "got: {err}");
    }

    #[test]
    fn given_bookmark_with_system_tag_when_add_second_system_tag_then_returns_error() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_snip_").unwrap());

        let mut bookmark = Bookmark::new("content", "title", "desc", tags).unwrap();
        let result = bookmark.add_system_tag(SystemTag::Shell);
        assert!(result.is_err());
        // Original tag should be preserved
        assert!(bookmark.is_snippet());
        assert!(!bookmark.is_shell());
    }

    #[test]
    fn given_multiple_known_system_tags_when_set_tags_then_returns_error() {
        let _ = init_test_env();
        let mut bookmark = Bookmark::new("content", "title", "desc", HashSet::new()).unwrap();

        let mut new_tags = HashSet::new();
        new_tags.insert(Tag::new("_md_").unwrap());
        new_tags.insert(Tag::new("_env_").unwrap());

        let result = bookmark.set_tags(new_tags);
        assert!(result.is_err());
        // Original tags should be preserved
        assert!(bookmark.tags.is_empty());
    }

    #[test]
    fn given_bookmark_with_system_tag_when_add_same_tag_then_ok() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_snip_").unwrap());

        let mut bookmark = Bookmark::new("content", "title", "desc", tags).unwrap();
        // Adding the same system tag again is idempotent (HashSet)
        let result = bookmark.add_tag(Tag::new("_snip_").unwrap());
        assert!(result.is_ok());
        assert!(bookmark.is_snippet());
    }

    #[test]
    fn given_single_system_tag_when_set_tags_then_ok() {
        let _ = init_test_env();
        let mut bookmark = Bookmark::new("content", "title", "desc", HashSet::new()).unwrap();

        let mut new_tags = HashSet::new();
        new_tags.insert(Tag::new("_shell_").unwrap());
        new_tags.insert(Tag::new("regular").unwrap());

        let result = bookmark.set_tags(new_tags);
        assert!(result.is_ok());
        assert!(bookmark.is_shell());
    }

    #[test]
    fn given_unknown_system_tags_when_new_then_allows_multiple() {
        let _ = init_test_env();
        // Unknown system tags (matching _xxx_ pattern but not in known list)
        // should not trigger the validation
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_custom1_").unwrap());
        tags.insert(Tag::new("_custom2_").unwrap());

        let result = Bookmark::new("content", "title", "desc", tags);
        assert!(result.is_ok());
    }
}
