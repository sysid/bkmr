// src/application/templates/bookmark_template.rs
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::domain::bookmark::{Bookmark, BookmarkBuilder};
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use derive_builder::Builder;
use std::collections::HashSet;
use tracing::instrument;

#[derive(Debug, Clone, Builder)]
#[builder(setter(into))]
pub struct BookmarkTemplate {
    #[builder(default)]
    pub id: Option<i32>,

    #[builder(default)]
    pub url: String,

    #[builder(default)]
    pub title: String,

    #[builder(default)]
    pub tags: HashSet<Tag>,

    #[builder(default)]
    pub comments: String,

    #[builder(default = "false")]
    pub embeddable: bool,
}

impl BookmarkTemplate {
    pub fn from_bookmark(bookmark: &Bookmark) -> Self {
        BookmarkTemplateBuilder::default()
            .id(bookmark.id)
            .url(bookmark.url.clone())
            .title(bookmark.title.clone())
            .tags(bookmark.tags.clone())
            .comments(bookmark.description.clone())
            .embeddable(bookmark.embeddable)
            .build()
            .unwrap()
    }

    pub fn new_empty() -> Self {
        BookmarkTemplateBuilder::default().build().unwrap()
    }

    pub fn for_type(system_tag: SystemTag) -> Self {
        let mut builder = BookmarkTemplateBuilder::default();

        // Set default values based on bookmark type
        match system_tag {
            SystemTag::Snippet => {
                builder
                    .url("// Enter your code snippet here")
                    .title("New Code Snippet")
                    .comments("Description of the snippet");

                // Build template first to get the tags
                let mut template = builder.build().unwrap();

                // Add the _snip_ tag
                if let Ok(tag) = Tag::new(SystemTag::Snippet.as_str()) {
                    template.tags.insert(tag);
                }

                return template;
            }
            SystemTag::Text => {
                builder
                    .url("Enter your text content here")
                    .title("New Text Document")
                    .comments("Description of the text document");

                // Build template first to get the tags
                let mut template = builder.build().unwrap();

                // Add the _imported_ tag
                if let Ok(tag) = Tag::new(SystemTag::Text.as_str()) {
                    template.tags.insert(tag);
                }

                return template;
            }
            SystemTag::Shell => {
                builder
                    .url("#!/bin/bash\n\n# Your shell script here\necho \"Hello World\"")
                    .title("New Shell Script")
                    .comments("Description of the shell script");

                // Build template first to get the tags
                let mut template = builder.build().unwrap();

                // Add the _shell_ tag
                if let Ok(tag) = Tag::new(SystemTag::Shell.as_str()) {
                    template.tags.insert(tag);
                }

                return template;
            }
            SystemTag::Markdown => {
                builder
                .url("# New Markdown Document\n\n## Introduction\n\nWrite your markdown content here.\n\n## Features\n\n- Lists\n- **Bold text**\n- *Italic text*\n- [Links](https://example.com)\n- Code blocks\n\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```")
                .title("New Markdown Document")
                .embeddable(true)
                .comments("Description of the markdown document");

                // Build template first to get the tags
                let mut template = builder.build().unwrap();

                // Add the _md_ tag
                if let Ok(tag) = Tag::new(SystemTag::Markdown.as_str()) {
                    template.tags.insert(tag);
                }

                return template;
            }
            SystemTag::Env => {
                builder
                .url("# Environment variables to be sourced\n# Usage: eval \"$(bkmr open <id>)\" or source <(bkmr open <id>)\n\nexport FOO=bar\nexport BAZ=qux\n\n# You can use interpolation too:\n# export DATE={{ current_date | strftime(\"%Y-%m-%d\") }}")
                .title("Environment Variables")
                .comments("Environment variables to be sourced in shell");

                // Build template first to get the tags
                let mut template = builder.build().unwrap();

                // Add the _env_ tag
                if let Ok(tag) = Tag::new(SystemTag::Env.as_str()) {
                    template.tags.insert(tag);
                }

                return template;
            }
            SystemTag::Uri => {
                builder
                    .url("https://")
                    .title("New Bookmark")
                    .comments("Enter description here");
                // No system tag for URI type
            }
        }

        builder.build().unwrap()
    }
    //noinspection RsExternalLinter
    pub fn to_string(&self) -> String {
        let tags_str = self
            .tags
            .iter()
            .map(|tag| tag.value().to_string())
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "# Bookmark Template\n\
            # Lines starting with '#' are comments and will be ignored.\n\
            # Section markers (=== SECTION_NAME ===) are required and must not be removed.\n\
            \n\
            === ID ===\n\
            {}\n\
            === URL ===\n\
            {}\n\
            === TITLE ===\n\
            {}\n\
            === TAGS ===\n\
            {}\n\
            === COMMENTS ===\n\
            {}\n\
            === EMBEDDABLE ===\n\
            {}\n\
            === END ===\n",
            self.id.map_or("".to_string(), |id| id.to_string()),
            self.url,
            self.title,
            tags_str,
            self.comments,
            if self.embeddable { "true" } else { "false" }
        )
    }

    // #[instrument(level = "debug", skip(content))]
    #[instrument(level = "debug")]
    pub fn from_string(content: &str) -> ApplicationResult<Self> {
        // Split the content by section markers
        let sections = parse_sections(content)?;

        // Extract id section
        let binding = String::new();
        let id_str = sections.get("ID").unwrap_or(&binding).trim();
        let id = if !id_str.is_empty() {
            Some(id_str.parse::<i32>().map_err(|_| {
                ApplicationError::Validation(format!("Invalid ID format: {}", id_str))
            })?)
        } else {
            None
        };

        // Extract URL section
        let binding = String::new();
        // let url = sections.get("URL").unwrap_or(&binding).trim_matches('\n');
        let url = sections.get("URL").unwrap_or(&binding).trim();
        if url.is_empty() {
            return Err(ApplicationError::Validation(
                "URL cannot be empty".to_string(),
            ));
        }

        // Extract title section
        let binding = String::new();
        let title = sections.get("TITLE").unwrap_or(&binding).trim();

        // Extract and parse tags
        let binding = String::new();
        let tags_str = sections.get("TAGS").unwrap_or(&binding).trim();
        let tags = if !tags_str.is_empty() {
            Tag::parse_tags(tags_str)
                .map_err(|e| ApplicationError::Validation(format!("Invalid tags format: {}", e)))?
        } else {
            HashSet::new()
        };

        // Extract comments (preserving whitespace)
        let comments = sections
            .get("COMMENTS")
            .unwrap_or(&String::new())
            .to_string();

        // Extract embeddable flag
        let binding = String::new();
        let embeddable_str = sections.get("EMBEDDABLE").unwrap_or(&binding).trim();
        let embeddable = match embeddable_str.to_lowercase().as_str() {
            "true" | "yes" | "1" => true,
            "false" | "no" | "0" | "" => false,
            _ => {
                return Err(ApplicationError::Validation(format!(
                    "Invalid embeddable format: {} (use true/false)",
                    embeddable_str
                )))
            }
        };

        Ok(Self {
            id,
            url: url.to_string(),
            title: title.to_string(),
            tags,
            comments,
            embeddable,
        })
    }

    #[instrument(level = "debug")]
    pub fn to_bookmark(&self, original: Option<&Bookmark>) -> ApplicationResult<Bookmark> {
        // Create a binding to extend the lifetime of the BookmarkBuilder
        let mut binding = BookmarkBuilder::default();
        let mut builder = binding
            .id(self.id)
            .url(&self.url)
            .title(&self.title)
            .description(&self.comments)
            .tags(self.tags.clone())
            .created_at(original.and_then(|b| b.created_at))
            .updated_at(chrono::Utc::now())
            .access_count(original.map_or(0, |b| b.access_count))
            .embeddable(self.embeddable);

        // Preserve embedding and content hash if available from original
        if let Some(bookmark) = original {
            builder = builder
                .embedding(bookmark.embedding.clone())
                .content_hash(bookmark.content_hash.clone());
        } else {
            // Initialize with None for new bookmarks
            builder = builder.embedding(None).content_hash(None);
        }

        builder
            .build()
            .map_err(|e| ApplicationError::Validation(e.to_string()))
    }
}

#[instrument(level = "trace")]
fn parse_sections(content: &str) -> ApplicationResult<std::collections::HashMap<String, String>> {
    let mut sections = std::collections::HashMap::new();

    // Use regex to match section markers: === SECTION_NAME ===
    // The markers must be at the start of a line (^) and must be main section names (ID, URL, etc.)
    // We restrict it to known section names to avoid matching content that looks like section markers
    let section_marker_regex =
        regex::Regex::new(r"(?m)^===\s+(ID|URL|TITLE|TAGS|COMMENTS|EMBEDDABLE|END)\s+===\s*$")
            .unwrap();

    // Find all section markers with their positions
    let mut markers: Vec<(usize, &str, &str)> = section_marker_regex
        .captures_iter(content)
        .map(|cap| {
            let full_match = cap.get(0).unwrap();
            let section_name = cap.get(1).unwrap().as_str();
            (full_match.start(), full_match.as_str(), section_name)
        })
        .collect();

    // Add a virtual end marker at the end of content
    markers.push((content.len(), "=== VIRTUAL_END ===", "VIRTUAL_END"));

    // Process each section
    for i in 0..markers.len() - 1 {
        let (start_pos, marker, section_name) = markers[i];
        let (next_pos, _, _) = markers[i + 1];

        // Skip END section
        if section_name == "END" {
            continue;
        }

        // Calculate the content start position (after the marker)
        let content_start = start_pos + marker.len();

        // Extract section content
        if content_start < next_pos {
            let section_content = &content[content_start..next_pos];
            // Trim just the leading and trailing newlines, but keep internal formatting
            // let trimmed = section_content.trim_start_matches('\n');
            let trimmed = section_content.trim_matches('\n');

            // Check if the section is actually empty
            if trimmed.is_empty() {
                sections.insert(section_name.to_string(), String::new());
            } else {
                // If not empty, preserve all content including internal newlines
                sections.insert(section_name.to_string(), trimmed.to_string());
            }
        } else {
            // Empty section
            sections.insert(section_name.to_string(), String::new());
        }
    }

    Ok(sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::init_test_env;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_template_roundtrip() {
        let _ = init_test_env();

        // Create a template
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());
        tags.insert(Tag::new("example").unwrap());

        let template = BookmarkTemplate {
            id: Some(123),
            url: "https://example.com\n\nanother text".to_string(),
            title: "  Stripped Example Site\n".to_string(),
            tags,
            comments: "This is a\nmultiline\ncomment".to_string(),
            embeddable: true,
        };

        // Convert to string
        let template_str = template.to_string();

        // Parse back
        let parsed = BookmarkTemplate::from_string(&template_str).unwrap();

        // Verify
        assert_eq!(parsed.id, Some(123));
        assert_eq!(parsed.url, "https://example.com\n\nanother text");
        assert_eq!(parsed.title, "Stripped Example Site");
        assert_eq!(parsed.tags.len(), 2);
        assert!(parsed.tags.iter().any(|t| t.value() == "test"));
        assert!(parsed.tags.iter().any(|t| t.value() == "example"));
        assert_eq!(parsed.comments, "This is a\nmultiline\ncomment");
    }

    #[test]
    #[serial]
    fn test_template_with_empty_lines() {
        let _ = init_test_env();

        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            Example Site\n\
            \n\
            With empty line\n\
            === TAGS ===\n\
            test,example\n\
            === COMMENTS ===\n\
            This is a comment\n\
            \n\
            with empty lines\n\
            \n\
            in between\n\
            === END ===\n";

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.title, "Example Site\n\nWith empty line");
        assert_eq!(parsed.tags.len(), 2);
        assert_eq!(
            parsed.comments,
            "This is a comment\n\nwith empty lines\n\nin between"
        );
    }

    #[test]
    #[serial]
    fn test_empty_id_creates_new_bookmark() {
        let _ = init_test_env();

        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            \n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            Example Site\n\
            === TAGS ===\n\
            test\n\
            === COMMENTS ===\n\
            This is a comment\n\
            === END ===\n";

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, None);
        assert_eq!(parsed.url, "https://example.com");
    }

    #[test]
    #[serial]
    fn test_invalid_tags_returns_error() {
        let _ = init_test_env();

        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            Example Site\n\
            === TAGS ===\n\
            invalid tag with space\n\
            === COMMENTS ===\n\
            This is a comment\n\
            === END ===\n";

        let result = BookmarkTemplate::from_string(template_str);

        assert!(result.is_err());
        if let Err(ApplicationError::Validation(msg)) = result {
            assert!(msg.contains("Invalid tags format"));
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    #[serial]
    fn test_missing_section_uses_default() {
        let _ = init_test_env();

        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            Example Site\n\
            === END ===\n"; // Missing TAGS and COMMENTS sections

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.title, "Example Site");
        assert_eq!(parsed.tags.len(), 0);
        assert_eq!(parsed.comments, "");
    }

    #[test]
    #[serial]
    fn test_complex_markdown_with_dashes() {
        let _ = init_test_env();

        // Create a template with complex markdown content that includes dashes
        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            # Markdown with Horizontal Rules\n\
            \n\
            This is a paragraph.\n\
            \n\
            ---\n\
            \n\
            This is another paragraph after a horizontal rule.\n\
            \n\
            ## Subheading\n\
            \n\
            - List item 1\n\
            - List item 2\n\
            - List item with --- dashes\n\
            \n\
            ```\n\
            code block with --- dashes\n\
            ```\n\
            === TITLE ===\n\
            Complex Markdown Test\n\
            === TAGS ===\n\
            markdown,test\n\
            === COMMENTS ===\n\
            Testing with complex content\n\
            === END ===\n";

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        assert!(parsed.url.contains("---"));
        assert!(parsed.url.contains("Markdown with Horizontal Rules"));
        assert_eq!(parsed.title, "Complex Markdown Test");
        assert_eq!(parsed.tags.len(), 2);
        assert!(parsed.tags.iter().any(|t| t.value() == "markdown"));
        assert!(parsed.tags.iter().any(|t| t.value() == "test"));
    }

    #[test]
    #[serial]
    fn test_content_with_section_like_text() {
        let _ = init_test_env();

        // Create a template with text that might be confused with section markers
        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            # Document with fake section markers\n\
            \n\
            This text has something that looks like === FAKE_SECTION ===\n\
            But it shouldn't be treated as a section marker.\n\
            \n\
            ```\n\
            === CODE_BLOCK ===\n\
            This is in a code block\n\
            ```\n\
            \n\
            And here's some more text.\n\
            === TITLE ===\n\
            Section Marker Test\n\
            === TAGS ===\n\
            test\n\
            === COMMENTS ===\n\
            Testing with content that looks like section markers\n\
            === END ===\n";

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        // Verify the content with fake section markers is preserved in the URL field
        assert!(parsed.url.contains("=== FAKE_SECTION ==="));
        assert!(parsed.url.contains("CODE_BLOCK"));
        assert!(parsed.url.contains("This is in a code block"));
        assert_eq!(parsed.title, "Section Marker Test");
    }

    #[test]
    #[serial]
    fn test_section_marker_at_end_of_file() {
        let _ = init_test_env();

        // Test with a section marker at the end of the file and no content after it
        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            End Test\n\
            === TAGS ===\n\
            test\n\
            === COMMENTS ===\n\
            Testing with END marker\n\
            === END ==="; // No newline after END marker

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.title, "End Test");
        assert_eq!(parsed.comments, "Testing with END marker");
    }

    #[test]
    #[serial]
    fn test_empty_sections() {
        let _ = init_test_env();

        // Test with some empty sections
        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            \n\
            === TAGS ===\n\
            \n\
            === COMMENTS ===\n\
            \n\
            === END ===\n";

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.title, "");
        assert_eq!(parsed.tags.len(), 0);
        assert_eq!(parsed.comments, "");
    }

    #[test]
    #[serial]
    fn test_adjacent_sections() {
        let _ = init_test_env();

        // Test with sections right next to each other
        let template_str = "\
            # Bookmark Template\n\
            === ID ===\n\
            123\n\
            === URL ===\n\
            https://example.com\n\
            === TITLE ===\n\
            Adjacent Test\n\
            === TAGS ===\n\
            test\n\
            === COMMENTS ===\n\
            === EMBEDDABLE ===\n\
            true\n\
            === END ===\n";

        let parsed = BookmarkTemplate::from_string(template_str).unwrap();

        assert_eq!(parsed.id, Some(123));
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.title, "Adjacent Test");
        assert_eq!(parsed.comments, "");
        assert!(parsed.embeddable);
    }
}
