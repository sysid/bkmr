// src/cli/display.rs (Updated)

use crate::domain::bookmark::Bookmark;
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayField {
    Id,
    Url,
    Title,
    Description,
    Tags,
    AccessCount,
    LastUpdateTs,
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
        }
    }
}

pub const DEFAULT_FIELDS: &[DisplayField] = &[
    DisplayField::Id,
    DisplayField::Title,
    DisplayField::Url,
    DisplayField::Tags,
];

pub const ALL_FIELDS: &[DisplayField] = &[
    DisplayField::Id,
    DisplayField::Title,
    DisplayField::Url,
    DisplayField::Description,
    DisplayField::Tags,
    DisplayField::AccessCount,
    DisplayField::LastUpdateTs,
];

#[derive(Debug, Clone)]
pub struct DisplayBookmark {
    pub id: i32,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: String,
    pub access_count: i32,
    pub last_update_ts: DateTime<Utc>,
    pub similarity: Option<f32>,
}

impl DisplayBookmark {
    pub fn new(
        id: i32,
        url: String,
        title: String,
        description: String,
        tags: String,
        access_count: i32,
        last_update_ts: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            url,
            title,
            description,
            tags,
            access_count,
            last_update_ts,
            similarity: None,
        }
    }

    pub fn from_domain(bookmark: &Bookmark) -> Self {
        Self {
            id: bookmark.id().unwrap_or(0),
            url: bookmark.url().to_string(),
            title: bookmark.title().to_string(),
            description: bookmark.description().to_string(),
            tags: bookmark.formatted_tags(),
            access_count: bookmark.access_count(),
            last_update_ts: bookmark.updated_at(),
            similarity: None,
        }
    }

    pub fn from_dto(dto: &crate::application::dto::BookmarkResponse) -> Self {
        Self {
            id: dto.id.unwrap_or(0),
            url: dto.url.clone(),
            title: dto.title.clone(),
            description: dto.description.clone(),
            tags: dto.tags.join(","),
            access_count: dto.access_count,
            last_update_ts: dto.updated_at,
            similarity: None,
        }
    }

    pub fn get_value(&self, field: &DisplayField) -> String {
        match field {
            DisplayField::Id => self.id.to_string(),
            DisplayField::Url => self.url.clone(),
            DisplayField::Title => self.title.clone(),
            DisplayField::Description => self.description.clone(),
            DisplayField::Tags => self.tags.clone(),
            DisplayField::AccessCount => self.access_count.to_string(),
            DisplayField::LastUpdateTs => self.last_update_ts.to_string(),
        }
    }
}

impl Default for DisplayBookmark {
    fn default() -> Self {
        Self {
            id: 0,
            url: String::new(),
            title: String::new(),
            description: String::new(),
            tags: String::new(),
            access_count: 0,
            last_update_ts: chrono::Utc::now(),
            similarity: None,
        }
    }
}

// Legacy compatibility
impl From<&crate::domain::bookmark::Bookmark> for DisplayBookmark {
    fn from(bookmark: &crate::domain::bookmark::Bookmark) -> Self {
        Self::from_domain(bookmark)
    }
}

pub fn show_bookmarks(bookmarks: &[DisplayBookmark], fields: &[DisplayField]) {
    if bookmarks.is_empty() {
        println!("No bookmarks to display");
        return;
    }

    // Determine column widths
    let mut col_widths: Vec<usize> = fields.iter().map(|f| f.to_string().len()).collect();

    for bm in bookmarks {
        for (i, field) in fields.iter().enumerate() {
            let value = bm.get_value(field);
            col_widths[i] = col_widths[i].max(value.len().min(50));
        }
    }

    // Print header
    for (i, field) in fields.iter().enumerate() {
        print!("{:width$} ", field, width = col_widths[i]);
    }
    println!();

    // Print separator
    for width in &col_widths {
        print!("{:-<width$} ", "", width = width);
    }
    println!();

    // Print bookmarks
    for bm in bookmarks {
        for (i, field) in fields.iter().enumerate() {
            let mut value = bm.get_value(field);

            // Truncate long values
            if value.len() > col_widths[i] {
                value.truncate(col_widths[i] - 3);
                value.push_str("...");
            }

            print!("{:width$} ", value, width = col_widths[i]);
        }

        // Print similarity if available
        if let Some(sim) = bm.similarity {
            print!(" (Similarity: {:.2})", sim);
        }

        println!();
    }
}
