// src/domain/tag.rs
use crate::domain::error::DomainError;
use crate::domain::tag::Tag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTag {
    Snippet,
    Text,
    Uri,
    Shell,
    Markdown,
    Env,
}

impl SystemTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Snippet => "_snip_",
            Self::Text => "_imported_", // todo: add a better name
            Self::Uri => "",
            Self::Shell => "_shell_",
            Self::Markdown => "_md_",
            Self::Env => "_env_",
        }
    }

    pub fn to_tag(&self) -> Result<Tag, DomainError> {
        Tag::new(self.as_str())
    }

    pub fn is_known_system_tag(tag_str: &str) -> bool {
        matches!(tag_str, "_snip_" | "_imported_" | "_shell_" | "_md_" | "_env_")
    }
}

impl Tag {
    pub fn is_system_tag(&self) -> bool {
        let val = self.value();
        val.starts_with('_') && val.ends_with('_')
    }
    pub fn is_known_system_tag(&self) -> bool {
        SystemTag::is_known_system_tag(self.value())
    }

    pub fn is_system_tag_of(&self, system_tag: SystemTag) -> bool {
        self.value() == system_tag.as_str()
    }
}

mod tests {
    use crate::app_state::AppState;
    use crate::domain::bookmark::Bookmark;
    use crate::domain::system_tag::SystemTag;
    use crate::domain::tag::Tag;
    use crate::util::testing::init_test_env;
    use std::collections::HashSet;

    #[test]
    fn given_tag_when_checking_is_known_system_tag_then_returns_correctly() {
        // arrange
        let known_system_tag = Tag::new("_snip_").unwrap();
        let unknown_system_tag = Tag::new("_unknown_").unwrap();
        let regular_tag = Tag::new("regular").unwrap();

        // act & assert
        assert!(known_system_tag.is_known_system_tag());
        assert!(!unknown_system_tag.is_known_system_tag());
        assert!(!regular_tag.is_known_system_tag());
    }

    #[test]
    fn test_system_tag_to_tag_conversion() {
        let system_tag = SystemTag::Snippet;
        let tag = system_tag.to_tag().unwrap();

        assert_eq!(tag.value(), "_snip_");
        assert!(tag.is_system_tag());
        assert!(tag.is_system_tag_of(SystemTag::Snippet));
    }

    #[test]
    fn test_tag_is_system_tag() {
        // System tag (enclosed with underscores)
        let system_tag = Tag::new("_snip_").unwrap();
        assert!(system_tag.is_system_tag());
        assert!(system_tag.is_system_tag_of(SystemTag::Snippet));

        // Regular tags
        let regular_tag1 = Tag::new("regular").unwrap();
        let regular_tag2 = Tag::new("_startsWithUnderscore").unwrap();
        let regular_tag3 = Tag::new("endsWithUnderscore_").unwrap();

        assert!(!regular_tag1.is_system_tag());
        assert!(!regular_tag2.is_system_tag());
        assert!(!regular_tag3.is_system_tag());

        // Not a recognized system tag
        let unknown_system_tag = Tag::new("_unknown_").unwrap();
        assert!(unknown_system_tag.is_system_tag());
        assert!(!unknown_system_tag.is_system_tag_of(SystemTag::Snippet));
    }

    #[test]
    fn test_bookmark_with_system_tags() {
        let _ = init_test_env();
        let mut tags = HashSet::new();

        // Add regular tags
        tags.insert(Tag::new("regular1").unwrap());
        tags.insert(Tag::new("regular2").unwrap());

        // Add system tag
        tags.insert(SystemTag::Snippet.to_tag().unwrap());

        let bookmark = Bookmark::new(
            "console.log('Hello, world!');", // For snippet, URL is the content
            "JavaScript Hello World",
            "A simple JavaScript snippet",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Test is_snippet
        assert!(bookmark.is_snippet());
        assert_eq!(bookmark.snippet_content(), "console.log('Hello, world!');");

        // Test get_system_tags
        let system_tags = bookmark.get_system_tags();
        assert_eq!(system_tags.len(), 1);
        assert!(system_tags.contains(&SystemTag::Snippet.to_tag().unwrap()));

        // Test get_tags
        let regular_tags = bookmark.get_tags();
        assert_eq!(regular_tags.len(), 2);
    }

    #[test]
    fn test_bookmark_add_remove_system_tag() {
        let _ = init_test_env();
        let mut tags = HashSet::new();
        tags.insert(Tag::new("regular").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags,
            AppState::read_global().context.embedder.as_ref(),
        )
        .unwrap();

        // Initially not a snippet
        assert!(!bookmark.is_snippet());

        // Add system tag
        bookmark.add_system_tag(SystemTag::Snippet).unwrap();
        assert!(bookmark.is_snippet());

        // Remove system tag
        bookmark.remove_system_tag(SystemTag::Snippet).unwrap();
        assert!(!bookmark.is_snippet());
    }
}
