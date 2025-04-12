// src/cli/display.rs

use crate::application::services::factory::create_interpolation_service;
use crate::domain::bookmark::Bookmark;
use crate::domain::search::SemanticSearchResult;
use chrono::{DateTime, Utc};
use derive_builder::Builder;
use std::fmt;
use std::io::{self, IsTerminal, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayField {
    Id,
    Url,
    Title,
    Description,
    Tags,
    AccessCount,
    LastUpdateTs,
    Similarity,
    Embedding,
    Embeddable,
}

impl fmt::Display for DisplayField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayField::Id => write!(f, "ID"),
            DisplayField::Url => write!(f, "URL"),
            DisplayField::Title => write!(f, "Title"),
            DisplayField::Description => write!(f, "Description"),
            DisplayField::Tags => write!(f, "Tags"),
            DisplayField::AccessCount => write!(f, "Access Count"),
            DisplayField::LastUpdateTs => write!(f, "Last Updated"),
            DisplayField::Similarity => write!(f, "Similarity"),
            DisplayField::Embedding => write!(f, "Embedding"),
            DisplayField::Embeddable => write!(f, "Embeddable"),
        }
    }
}

pub const DEFAULT_FIELDS: &[DisplayField] = &[
    DisplayField::Id,
    DisplayField::Url,
    DisplayField::Title,
    DisplayField::Description,
    DisplayField::Tags,
    DisplayField::Similarity,
];

pub const ALL_FIELDS: &[DisplayField] = &[
    DisplayField::Id,
    DisplayField::Url,
    DisplayField::Title,
    DisplayField::Description,
    DisplayField::Tags,
    DisplayField::AccessCount,
    DisplayField::LastUpdateTs,
    DisplayField::Similarity,
    DisplayField::Embedding,
    DisplayField::Embeddable,
];

#[derive(Debug, Clone, Builder)]
#[builder(setter(into))]
pub struct DisplayBookmark {
    #[builder(default = "0")]
    pub id: i32,

    #[builder(default)]
    pub url: String,

    #[builder(default)]
    pub title: String,

    #[builder(default)]
    pub description: String,

    #[builder(default)]
    pub tags: String,

    #[builder(default = "0")]
    pub access_count: i32,

    #[builder(default = "chrono::Utc::now()")]
    pub last_update_ts: DateTime<Utc>,

    #[builder(default)]
    pub similarity: Option<f32>,

    #[builder(default)]
    pub embedding: String,

    #[builder(default = "false")]
    pub embeddable: bool,
}

impl DisplayBookmark {
    pub fn from_domain(bookmark: &Bookmark) -> Self {
        // Try to render the URL if it contains interpolation markers
        let interpolation_service = create_interpolation_service();
        let url = if bookmark.url.contains("{{") || bookmark.url.contains("{%") {
            match interpolation_service.render_bookmark_url(bookmark) {
                Ok(rendered) => rendered,
                Err(_) => bookmark.url.clone(), // Fallback to original URL if rendering fails
            }
        } else {
            bookmark.url.clone()
        };

        DisplayBookmarkBuilder::default()
            .id(bookmark.id.unwrap_or(0))
            .url(url)
            .title(bookmark.title.to_string())
            .description(bookmark.description.to_string())
            .tags(bookmark.formatted_tags())
            .access_count(bookmark.access_count)
            .last_update_ts(bookmark.updated_at)
            .embedding(
                bookmark
                    .embedding
                    .as_ref()
                    .map_or_else(String::new, |_| "yes".to_string()),
            )
            .embeddable(bookmark.embeddable)
            .build()
            .unwrap()
    }

    pub fn get_value(&self, field: &DisplayField) -> String {
        match field {
            DisplayField::Embeddable => if self.embeddable { "yes" } else { "no" }.to_string(),
            DisplayField::Id => self.id.to_string(),
            DisplayField::Url => self.url.clone(),
            DisplayField::Title => self.title.clone(),
            DisplayField::Description => self.description.clone(),
            DisplayField::Tags => self.tags.clone(),
            DisplayField::AccessCount => self.access_count.to_string(),
            DisplayField::LastUpdateTs => self.last_update_ts.to_string(),
            DisplayField::Similarity => self.similarity.map_or_else(String::new, |s| s.to_string()),
            DisplayField::Embedding => self.embedding.clone(),
        }
    }
    pub fn from_semantic_result(result: &SemanticSearchResult) -> Self {
        let mut builder = DisplayBookmarkBuilder::default();

        // Start with the base bookmark fields
        let base = Self::from_domain(&result.bookmark);

        // Build with all the base fields plus similarity
        builder
            .id(base.id)
            .url(base.url)
            .title(base.title)
            .description(base.description)
            .tags(base.tags)
            .access_count(base.access_count)
            .last_update_ts(base.last_update_ts)
            .embedding(base.embedding)
            .embeddable(base.embeddable)
            .similarity(Some(result.similarity))
            .build()
            .unwrap()
    }
}

// Implement Default directly instead of deriving it,
// as we already provide defaults in the builder
impl Default for DisplayBookmark {
    fn default() -> Self {
        Self {
            id: 0,
            url: String::new(),
            title: String::new(),
            description: String::new(),
            tags: String::new(),
            access_count: 0,
            last_update_ts: Utc::now(),
            similarity: None,
            embedding: String::new(),
            embeddable: false,
        }
    }
}

/// Display bookmarks with color formatting
pub fn show_bookmarks(bookmarks: &[DisplayBookmark], fields: &[DisplayField]) {
    if bookmarks.is_empty() {
        eprintln!("No bookmarks to display");
        return;
    }

    // Check if the output is a TTY
    let color_choice = if io::stderr().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    let mut stderr = StandardStream::stderr(color_choice);
    let first_col_width = bookmarks.len().to_string().len();

    for (i, bm) in bookmarks.iter().enumerate() {
        // Title/Metadata (green)
        if fields.contains(&DisplayField::Title) {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Green))) {
                eprintln!("Error setting color: {}", e);
            }
            if let Err(e) = write!(&mut stderr, "{:first_col_width$}. {}", i + 1, bm.title) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // Similarity score if available
        if let Some(similarity) = bm.similarity {
            if fields.contains(&DisplayField::Similarity) {
                if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::White))) {
                    eprintln!("Error setting color: {}", e);
                }
                if let Err(e) = write!(&mut stderr, " [{:.3}]", similarity) {
                    eprintln!("Error writing to stderr: {}", e);
                }
            }
        }

        // ID (white)
        if fields.contains(&DisplayField::Id) {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::White))) {
                eprintln!("Error setting color: {}", e);
            }
            if let Err(e) = writeln!(&mut stderr, " [{}]", bm.id) {
                eprintln!("Error writing to stderr: {}", e);
            }
        } else {
            // End the title line if no ID is shown
            if let Err(e) = writeln!(&mut stderr) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // URL (yellow)
        if fields.contains(&DisplayField::Url) {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Yellow))) {
                eprintln!("Error setting color: {}", e);
            }
            // Format multiline URL for display
            let formatted_url = if bm.url.contains('\n') {
                bm.url.replace('\n', "\n    ") // Add proper indentation for each line
            } else {
                bm.url.clone()
            };
            if let Err(e) = writeln!(&mut stderr, "{:first_col_width$}  {}", "", formatted_url) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // Description (white)
        if fields.contains(&DisplayField::Description) && !bm.description.is_empty() {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::White))) {
                eprintln!("Error setting color: {}", e);
            }
            if let Err(e) = writeln!(&mut stderr, "{:first_col_width$}  {}", "", bm.description) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // Tags (blue)
        if fields.contains(&DisplayField::Tags) {
            let tags = bm.tags.replace(',', " ");
            if tags.find(|c: char| !c.is_whitespace()).is_some() {
                if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Blue))) {
                    eprintln!("Error setting color: {}", e);
                }
                if let Err(e) = writeln!(&mut stderr, "{:first_col_width$}  {}", "", tags.trim()) {
                    eprintln!("Error writing to stderr: {}", e);
                }
            }
        }

        // Access count and embedding status
        let mut flags_and_embedding_line = String::new();

        if fields.contains(&DisplayField::AccessCount) {
            flags_and_embedding_line.push_str(&format!("Count: {}", bm.access_count));
        }

        if fields.contains(&DisplayField::Embedding) {
            let embed_status = if bm.embedding.is_empty() {
                "null"
            } else {
                "yes"
            };
            if !flags_and_embedding_line.is_empty() {
                flags_and_embedding_line.push_str(" | ");
            }
            flags_and_embedding_line.push_str(&format!("embed: {}", embed_status));
        }

        // Add embeddable status to the display
        if fields.contains(&DisplayField::Embeddable) {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::White))) {
                eprintln!("Error setting color: {}", e);
            }
            if let Err(e) = writeln!(
                &mut stderr,
                "{:first_col_width$}  Embeddable: {}",
                "",
                if bm.embeddable { "yes" } else { "no" }
            ) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // Print access count and embedding info if any exist
        if !flags_and_embedding_line.is_empty() {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::White))) {
                eprintln!("Error setting color: {}", e);
            }
            if let Err(e) = writeln!(
                &mut stderr,
                "{:first_col_width$}  {}",
                "", flags_and_embedding_line
            ) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // Last update timestamp (magenta)
        if fields.contains(&DisplayField::LastUpdateTs) {
            if let Err(e) = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Magenta))) {
                eprintln!("Error setting color: {}", e);
            }
            if let Err(e) = writeln!(
                &mut stderr,
                "{:first_col_width$}  {}",
                "", bm.last_update_ts
            ) {
                eprintln!("Error writing to stderr: {}", e);
            }
        }

        // Reset colors and print a blank line between bookmarks
        if let Err(e) = stderr.reset() {
            eprintln!("Error resetting color: {}", e);
        }
        eprintln!();
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    use chrono::Utc;
    use serial_test::serial;
    use std::{fs, path::Path};

    fn create_test_bookmarks() -> Vec<DisplayBookmark> {
        vec![
            DisplayBookmark {
                id: 1,
                url: "https://www.rust-lang.org".to_string(),
                title: "The Rust Programming Language".to_string(),
                description:
                    "A language empowering everyone to build reliable and efficient software."
                        .to_string(),
                tags: ",rust,programming,systems,".to_string(),
                access_count: 42,
                last_update_ts: Utc::now(),
                similarity: Some(0.85),
                embedding: "yes".to_string(),
                embeddable: true,
            },
            DisplayBookmark {
                id: 2,
                url: "https://doc.rust-lang.org/book/".to_string(),
                title: "The Rust Book".to_string(),
                description: "The Rust Programming Language Book".to_string(),
                tags: ",book,documentation,rust,learning,".to_string(),
                access_count: 24,
                last_update_ts: Utc::now(),
                similarity: None,
                embedding: "".to_string(),
                embeddable: false,
            },
            DisplayBookmark {
                id: 3,
                url: "https://crates.io".to_string(),
                title: "Rust Package Registry".to_string(),
                description: "".to_string(), // Empty description
                tags: ",crates,registry,".to_string(),
                access_count: 12,
                last_update_ts: Utc::now(),
                similarity: Some(0.62),
                embedding: "yes".to_string(),
                embeddable: true,
            },
        ]
    }

    #[test]
    #[serial]
    fn test_show_bookmarks_visual() {
        println!("\n\nTEST: Colored Bookmark Display - Default Fields\n");
        let bookmarks = create_test_bookmarks();
        show_bookmarks(&bookmarks, DEFAULT_FIELDS);
    }

    #[test]
    #[serial]
    fn test_show_bookmarks_visual_all_fields() {
        println!("\n\nTEST: Colored Bookmark Display - All Fields\n");
        let bookmarks = create_test_bookmarks();

        // Create a version of ALL_FIELDS that includes Similarity and Embedding
        let extended_fields = &[
            DisplayField::Id,
            DisplayField::Title,
            DisplayField::Url,
            DisplayField::Description,
            DisplayField::Tags,
            DisplayField::AccessCount,
            DisplayField::LastUpdateTs,
            DisplayField::Similarity,
            DisplayField::Embedding,
        ];

        show_bookmarks(&bookmarks, extended_fields);
    }

    #[test]
    #[serial]
    fn test_show_bookmarks_empty() {
        println!("\n\nTEST: Empty Bookmark List\n");
        let empty_bookmarks: Vec<DisplayBookmark> = Vec::new();
        show_bookmarks(&empty_bookmarks, DEFAULT_FIELDS);
    }

    #[test]
    #[serial]
    fn test_output_to_file() -> io::Result<()> {
        use std::io::Write;

        // Create output directory if it doesn't exist
        let output_dir = Path::new("target").join("display_test_output");
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir)?;
        }

        // Redirect stdout to a file temporarily
        let mut output_file = fs::File::create(output_dir.join("bookmarks_display_test.txt"))?;

        // We can't redirect stderr where the colored output goes, but we can still save
        // the table output for inspection
        let bookmarks = create_test_bookmarks();

        // Write to the file
        writeln!(output_file, "=== BOOKMARK TABLE FORMAT ===")?;

        // We can't easily redirect the show_bookmarks output since it uses stderr
        // but we can create similar output manually for the table version
        for (_, field) in DEFAULT_FIELDS.iter().enumerate() {
            write!(output_file, "{} ", field)?;
        }
        writeln!(output_file)?;

        for bm in &bookmarks {
            for field in DEFAULT_FIELDS {
                let value = bm.get_value(field);
                write!(output_file, "{} ", value)?;
            }
            writeln!(output_file)?;
        }

        println!(
            "Display test output saved to: {}",
            output_dir.join("bookmarks_display_test.txt").display()
        );

        Ok(())
    }
}
