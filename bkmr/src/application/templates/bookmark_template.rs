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

    #[builder(default)]
    pub opener: Option<String>,
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
            .opener(bookmark.opener.clone())
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
                    .url("#!/usr/bin/env bash\n\n# Your shell script here\necho \"Hello World\"")
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
            SystemTag::Memory => {
                builder
                    .url("Enter memory content here")
                    .title("Memory Title")
                    .comments("");

                let mut template = builder.build().unwrap();

                if let Ok(tag) = Tag::new(SystemTag::Memory.as_str()) {
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
            # ID: clear it to save a new bookmark; another ID is refused.\n\
            # EMBEDDABLE: true or false; a type set to \"always\" in [embeddings.policy] stays true.\n\
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
            === OPENER ===\n\
            {}\n\
            === END ===\n",
            self.id.map_or("".to_string(), |id| id.to_string()),
            self.url,
            self.title,
            tags_str,
            self.comments,
            if self.embeddable { "true" } else { "false" },
            self.opener.as_deref().unwrap_or("")
        )
    }

    // Content stays out of the span: it can hold secrets, e.g. an _env_ snippet
    #[instrument(level = "debug", skip_all)]
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
        // Leading whitespace is content: the indentation of a YAML item or a Python block
        let url = sections.get("URL").unwrap_or(&binding).trim_end();
        if url.trim().is_empty() {
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

        // Extract opener (custom open command)
        let binding = String::new();
        let opener_str = sections.get("OPENER").unwrap_or(&binding).trim();
        let opener = if opener_str.is_empty() {
            None
        } else {
            Some(opener_str.to_string())
        };

        Ok(Self {
            id,
            url: url.to_string(),
            title: title.to_string(),
            tags,
            comments,
            embeddable,
            opener,
        })
    }

    #[instrument(level = "debug", skip_all)]
    pub fn to_bookmark(&self, original: Option<&Bookmark>) -> ApplicationResult<Bookmark> {
        // The ID decides which row the edit writes: another one would overwrite that bookmark.
        // Clearing it is allowed, and saves the edit as a new bookmark.
        if let Some(original) = original {
            if self.id.is_some() && self.id != original.id {
                return Err(ApplicationError::Validation(format!(
                    "The ID cannot be changed in the editor (was {}, now {})",
                    original.id.map_or("empty".to_string(), |id| id.to_string()),
                    self.id.map_or("empty".to_string(), |id| id.to_string()),
                )));
            }
        }

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
            .embeddable(self.embeddable)
            .opener(self.opener.clone())
            .accessed_at(original.and_then(|b| b.accessed_at));

        // Preserve embedding, content hash and the link to an imported source file. The form
        // does not show the file fields, and the repository writes a missing one as NULL.
        if let Some(bookmark) = original {
            builder = builder
                .embedding(bookmark.embedding.clone())
                .content_hash(bookmark.content_hash.clone())
                .file_path(bookmark.file_path.clone())
                .file_mtime(bookmark.file_mtime)
                .file_hash(bookmark.file_hash.clone());
        } else {
            // Initialize with None for new bookmarks
            builder = builder.embedding(None).content_hash(None);
        }

        builder
            .build()
            .map_err(|e| ApplicationError::Validation(e.to_string()))
    }
}

#[instrument(level = "trace", skip_all)]
fn parse_sections(content: &str) -> ApplicationResult<std::collections::HashMap<String, String>> {
    let mut sections = std::collections::HashMap::new();

    // Use regex to match section markers: === SECTION_NAME ===
    // The markers must be at the start of a line (^) and must be main section names (ID, URL, etc.)
    // We restrict it to known section names to avoid matching content that looks like section markers
    let section_marker_regex = regex::Regex::new(
        r"(?m)^===\s+(ID|URL|TITLE|TAGS|COMMENTS|EMBEDDABLE|OPENER|END)\s+===\s*$",
    )
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

    #[test]
    fn given_template_with_two_system_tags_when_to_bookmark_then_returns_error() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());
        tags.insert(Tag::new("_mem_").unwrap());
        let template = BookmarkTemplate {
            id: Some(1),
            url: "# memory".to_string(),
            title: "title".to_string(),
            tags,
            comments: String::new(),
            embeddable: true,
            opener: None,
        };

        let result = template.to_bookmark(None);

        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("at most one system tag"), "got: {err}");
    }

    #[test]
    fn given_file_backed_original_when_to_bookmark_then_keeps_link_to_source_file() {
        // Arrange: a bookmark imported from a file, edited in the database (edit --force-db)
        let mut original = Bookmark::new("echo hi", "deploy", "", HashSet::new()).unwrap();
        original.set_id(12);
        original.file_path = Some("$SCRIPTS/deploy.sh".to_string());
        original.file_mtime = Some(1_700_000_000);
        original.file_hash = Some("abc123".to_string());
        let mut template = BookmarkTemplate::from_bookmark(&original);
        template.title = "deploy prod".to_string();

        // Act
        let bookmark = template.to_bookmark(Some(&original)).unwrap();

        // Assert
        assert_eq!(bookmark.file_path.as_deref(), Some("$SCRIPTS/deploy.sh"));
        assert_eq!(bookmark.file_mtime, Some(1_700_000_000));
        assert_eq!(bookmark.file_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn given_id_changed_in_editor_when_to_bookmark_then_refuses_to_write_another_bookmark() {
        // Arrange: bkmr edit 12, and the user types 34 into the ID section
        let mut original = Bookmark::new("https://a.example", "a", "", HashSet::new()).unwrap();
        original.set_id(12);
        let mut template = BookmarkTemplate::from_bookmark(&original);
        template.id = Some(34);
        template.url = "https://b.example".to_string();

        // Act
        let result = template.to_bookmark(Some(&original));

        // Assert
        let err = result
            .expect_err("a changed ID must be refused")
            .to_string();
        assert!(err.contains("ID cannot be changed"), "got: {err}");
    }

    #[test]
    fn given_id_cleared_in_editor_when_to_bookmark_then_returns_new_bookmark() {
        // Arrange: bkmr edit 12, and the user empties the ID section to save a copy
        let mut original = Bookmark::new("https://a.example", "a", "", HashSet::new()).unwrap();
        original.set_id(12);
        let mut template = BookmarkTemplate::from_bookmark(&original);
        template.id = None;

        // Act
        let bookmark = template.to_bookmark(Some(&original)).unwrap();

        // Assert
        assert_eq!(bookmark.id, None);
    }

    #[test]
    fn given_indented_first_line_when_round_trip_then_keeps_indentation() {
        // Arrange: a YAML list item or an indented Python block, edited only in its title
        let template = BookmarkTemplate {
            id: Some(7),
            url: "  - name: a\n    value: b".to_string(),
            title: "yaml".to_string(),
            tags: HashSet::new(),
            comments: String::new(),
            embeddable: false,
            opener: None,
        };

        // Act
        let parsed = BookmarkTemplate::from_string(&template.to_string()).unwrap();

        // Assert
        assert_eq!(parsed.url, "  - name: a\n    value: b");
    }

    #[test]
    fn given_only_whitespace_as_url_when_parse_then_rejects_it() {
        let template = BookmarkTemplate {
            id: None,
            url: "   ".to_string(),
            title: "blank".to_string(),
            tags: HashSet::new(),
            comments: String::new(),
            embeddable: false,
            opener: None,
        };

        let result = BookmarkTemplate::from_string(&template.to_string());

        assert!(result.is_err());
    }

    #[test]
    fn given_bookmark_when_convert_to_template_and_back_then_preserves_data() {
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
            opener: None,
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
    fn given_template_with_empty_lines_when_parse_then_handles_correctly() {
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
    fn given_template_with_empty_id_when_parse_then_creates_new_bookmark() {
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
    fn given_template_with_invalid_tags_when_parse_then_returns_error() {
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
    fn given_template_with_missing_section_when_parse_then_uses_default() {
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
    fn given_complex_markdown_with_dashes_when_parse_then_handles_correctly() {
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
    fn given_content_with_section_markers_when_parse_then_distinguishes_sections() {
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
    fn given_section_marker_at_file_end_when_parse_then_handles_correctly() {
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
    fn given_template_with_empty_sections_when_parse_then_uses_defaults() {
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
    fn given_template_with_adjacent_sections_when_parse_then_handles_correctly() {
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
