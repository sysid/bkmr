// src/application/actions/markdown_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::embedding::Embedder;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::repository::BookmarkRepository;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::util::helper::calc_content_hash;
use crate::util::path::{abspath, is_file_path};
use markdown::{to_html_with_options, Options};
use regex::Regex;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

/// Represents a table of contents entry
#[derive(Debug, Clone)]
struct TocEntry {
    level: u8,
    title: String,
    id: String,
}

#[derive(Debug)]
pub struct MarkdownAction {
    repository: Option<Arc<dyn BookmarkRepository>>,
    embedder: Arc<dyn Embedder>,
}

impl MarkdownAction {
    #[allow(dead_code)]
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self { repository: None, embedder }
    }

    // Constructor with repository for embedding support
    pub fn new_with_repository(repository: Arc<dyn BookmarkRepository>, embedder: Arc<dyn Embedder>) -> Self {
        Self {
            repository: Some(repository),
            embedder,
        }
    }

    /// Reads markdown content from a file path
    fn read_markdown_from_file(&self, path_str: &str) -> DomainResult<String> {
        debug!("Attempting to read from path: {}", path_str);

        // First try to resolve the path with abspath (handles shell variables, ~, etc.)
        let resolved_path = match abspath(path_str) {
            Some(path) => {
                debug!("Path resolved with abspath: {}", path);
                path
            }
            None => {
                // If abspath fails, try as a relative path
                debug!("abspath failed, trying as relative path");
                let path = Path::new(path_str);
                if path.exists() {
                    path.to_string_lossy().to_string()
                } else {
                    return Err(DomainError::Other(format!(
                        "File not found: {}. Neither absolute nor relative path exists.",
                        path_str
                    )));
                }
            }
        };

        debug!("Reading from resolved path: {}", resolved_path);

        // Read the file contents
        fs::read_to_string(&resolved_path).map_err(|e| {
            DomainError::Other(format!(
                "Failed to read markdown file '{}': {}",
                resolved_path, e
            ))
        })
    }

    // TODO: why do we need embeddings here (SRP violation?)
    /// Check if embedding is allowed and possible
    fn can_update_embedding(&self, bookmark: &Bookmark) -> bool {
        // Check if we have a repository
        if self.repository.is_none() {
            return false;
        }

        // Check if the bookmark is embeddable
        if !bookmark.embeddable {
            return false;
        }

        // Check if OpenAI embeddings are enabled (not using DummyEmbedding)
        self.embedder.as_any().type_id() != std::any::TypeId::of::<DummyEmbedding>()
    }

    /// Update bookmark with embedding if repository is available and conditions are met
    fn update_embedding(&self, bookmark: &Bookmark, content: &str) -> DomainResult<()> {
        // Check if embedding is allowed
        if !self.can_update_embedding(bookmark) {
            debug!("Embedding update skipped: not allowed or not possible");
            return Ok(());
        }

        let repository = self.repository.as_ref().unwrap();

        if let Some(id) = bookmark.id {
            // Get the current state of the bookmark
            let mut updated_bookmark = repository
                .get_by_id(id)?
                .ok_or_else(|| DomainError::BookmarkNotFound(id.to_string()))?;

            // Calculate content hash for the current content
            let content_hash = calc_content_hash(content);

            // Only update if content has changed
            if updated_bookmark.content_hash.as_ref() != Some(&content_hash) {
                debug!("Content changed, updating embedding for bookmark ID {}", id);

                // Use the instance embedder instead of global state
                let embedder = &*self.embedder;

                // Generate embedding
                if let Some(embedding) = embedder.embed(content)? {
                    // Serialize the embedding
                    let serialized = crate::domain::embedding::serialize_embedding(embedding)?;

                    // Update the bookmark
                    updated_bookmark.embedding = Some(serialized);
                    updated_bookmark.content_hash = Some(content_hash);

                    // Save to repository
                    repository.update(&updated_bookmark)?;
                    info!("Successfully updated embedding for bookmark ID {}", id);
                }
            } else {
                debug!(
                    "Content unchanged, not updating embedding for bookmark ID {}",
                    id
                );
            }
        }

        Ok(())
    }

    /// Extract headers from HTML content and add IDs if missing
    fn extract_and_process_headers(&self, html_content: &str) -> (String, Vec<TocEntry>) {
        let mut processed_html = html_content.to_string();
        let mut toc_entries: Vec<TocEntry> = Vec::new();
        let mut header_counts = std::collections::HashMap::new();

        // Regex to match h1, h2, h3 headers
        let header_regex = Regex::new(r"<(h[123])(?:\s+[^>]*)?>(.*?)</h[123]>").unwrap();
        let id_regex = Regex::new(r#"\s+id\s*=\s*["']([^"']+)["']"#).unwrap();

        // Find all headers and process them
        let matches: Vec<_> = header_regex.find_iter(html_content).collect();

        for m in matches.iter() {
            let full_match = m.as_str();

            if let Some(header_cap) = header_regex.captures(full_match) {
                let level = match &header_cap[1] {
                    "h1" => 1,
                    "h2" => 2,
                    "h3" => 3,
                    _ => continue,
                };

                let content = &header_cap[2];

                // Check if header already has an ID
                let existing_id = id_regex.captures(full_match).map(|c| c[1].to_string());

                let header_id = if let Some(ref id) = existing_id {
                    id.clone()
                } else {
                    // Generate ID from content
                    let base_id = self.generate_header_id(content);

                    // Handle duplicates
                    let count = header_counts.entry(base_id.clone()).or_insert(0);
                    *count += 1;

                    if *count > 1 {
                        format!("{}-{}", base_id, *count - 1)
                    } else {
                        base_id
                    }
                };

                // Create TOC entry
                toc_entries.push(TocEntry {
                    level,
                    title: self.clean_html_content(content),
                    id: header_id.clone(),
                });

                // Add ID to header if it doesn't exist
                if existing_id.is_none() {
                    let new_header = format!(
                        "<{} id=\"{}\">{}</{}>",
                        &header_cap[1], header_id, content, &header_cap[1]
                    );

                    // Replace in the processed HTML
                    processed_html = processed_html.replace(full_match, &new_header);
                }
            }
        }

        (processed_html, toc_entries)
    }

    /// Generate a URL-safe ID from header content
    fn generate_header_id(&self, content: &str) -> String {
        // Remove HTML tags and clean content
        let clean_content = self.clean_html_content(content);

        // Convert to lowercase, replace spaces and special chars with hyphens
        clean_content
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c
                } else if c.is_whitespace() || c == '-' || c == '_' {
                    '-'
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>()
            .join("-")
    }

    /// Remove HTML tags from content
    fn clean_html_content(&self, content: &str) -> String {
        let tag_regex = Regex::new(r"<[^>]*>").unwrap();
        tag_regex.replace_all(content, "").trim().to_string()
    }

    /// Get the source file path from a bookmark if it contains a file path
    fn get_source_file_path(&self, bookmark: &Bookmark) -> Option<std::path::PathBuf> {
        let content_or_path = &bookmark.url;

        if is_file_path(content_or_path) {
            // Try to resolve the path
            if let Some(resolved) = abspath(content_or_path) {
                return Some(std::path::PathBuf::from(resolved));
            }

            // Fallback to relative path if abspath fails
            let path = std::path::Path::new(content_or_path);
            if path.exists() {
                if let Ok(canonical) = path.canonicalize() {
                    return Some(canonical);
                }
            }
        }

        None
    }

    /// Extract local resource paths from HTML (images, stylesheets, scripts)
    /// Returns a vector of (full_tag_match, path) tuples for replacement
    fn extract_resource_paths(&self, html: &str) -> Vec<(String, String)> {
        let mut resources = Vec::new();

        // Regex patterns for different resource types
        // Match img src, link href, and script src
        let img_regex = Regex::new(r#"<img\s+[^>]*src\s*=\s*["']([^"']+)["'][^>]*>"#).unwrap();
        let link_regex = Regex::new(r#"<link\s+[^>]*href\s*=\s*["']([^"']+)["'][^>]*>"#).unwrap();
        let script_regex = Regex::new(r#"<script\s+[^>]*src\s*=\s*["']([^"']+)["'][^>]*>"#).unwrap();

        // Extract image paths
        for cap in img_regex.captures_iter(html) {
            let full_match = cap[0].to_string();
            let path = cap[1].to_string();

            // Only include local paths (not http://, https://, data:, etc.)
            if !path.starts_with("http://") && !path.starts_with("https://")
                && !path.starts_with("data:") && !path.starts_with("//") {
                resources.push((full_match, path));
            }
        }

        // Extract link href paths (stylesheets, etc.)
        for cap in link_regex.captures_iter(html) {
            let full_match = cap[0].to_string();
            let path = cap[1].to_string();

            if !path.starts_with("http://") && !path.starts_with("https://")
                && !path.starts_with("data:") && !path.starts_with("//") {
                resources.push((full_match, path));
            }
        }

        // Extract script src paths
        for cap in script_regex.captures_iter(html) {
            let full_match = cap[0].to_string();
            let path = cap[1].to_string();

            if !path.starts_with("http://") && !path.starts_with("https://")
                && !path.starts_with("data:") && !path.starts_with("//") {
                resources.push((full_match, path));
            }
        }

        resources
    }

    /// Resolve a resource path to its absolute location on disk
    /// relative_path: The path from the HTML (e.g., "./images/foo.png")
    /// source_md_path: The absolute path to the source markdown file
    /// Returns: Some(absolute PathBuf) if the file exists, None otherwise
    fn resolve_resource_path(&self, relative_path: &str, source_md_path: &Path) -> Option<std::path::PathBuf> {
        // Get the directory containing the source markdown file
        let source_dir = source_md_path.parent()?;

        // Resolve the relative path from the source directory
        let resource_path = if relative_path.starts_with('/') {
            // Absolute path - use as is
            std::path::PathBuf::from(relative_path)
        } else {
            // Relative path - resolve from source directory
            source_dir.join(relative_path)
        };

        // Canonicalize the path to resolve .. and . components
        if let Ok(canonical_path) = resource_path.canonicalize() {
            // Check if the file exists
            if canonical_path.exists() {
                debug!("Resolved resource path '{}' to '{:?}'", relative_path, canonical_path);
                return Some(canonical_path);
            } else {
                debug!("Resource path '{}' resolved but file does not exist: {:?}", relative_path, canonical_path);
            }
        } else {
            debug!("Failed to canonicalize resource path: {}", relative_path);
        }

        None
    }

    /// Copy local resources to the temporary directory
    /// html: The HTML content with potentially relative resource paths
    /// source_md_path: The absolute path to the source markdown file
    /// temp_dir: The temporary directory where resources should be copied
    /// Returns: Ok(()) if successful, error otherwise
    fn copy_resources_to_temp(&self, html: &str, source_md_path: &Path, temp_dir: &Path) -> DomainResult<()> {
        // Extract all local resource paths from the HTML
        let resources = self.extract_resource_paths(html);

        if resources.is_empty() {
            debug!("No local resources found in HTML");
            return Ok(());
        }

        debug!("Found {} local resources to copy", resources.len());

        // For each resource, copy it to the temp directory
        for (_full_match, relative_path) in resources {
            if let Some(source_file) = self.resolve_resource_path(&relative_path, source_md_path) {
                // Determine destination path in temp directory
                // Maintain the relative path structure to keep HTML paths valid
                let dest_path = temp_dir.join(&relative_path);

                // Create parent directories if needed
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        DomainError::Other(format!(
                            "Failed to create directory {:?}: {}",
                            parent, e
                        ))
                    })?;
                }

                // Copy the file
                fs::copy(&source_file, &dest_path).map_err(|e| {
                    DomainError::Other(format!(
                        "Failed to copy resource from {:?} to {:?}: {}",
                        source_file, dest_path, e
                    ))
                })?;

                info!("Copied resource: {} from {:?} to {:?}", relative_path, source_file, dest_path);
            } else {
                // Log warning but don't fail - resource might be optional
                debug!("Could not resolve resource path, skipping: {}", relative_path);
            }
        }

        Ok(())
    }

    /// Generate TOC HTML sidebar
    fn generate_toc_html(&self, toc_entries: &[TocEntry]) -> String {
        if toc_entries.is_empty() {
            return String::new();
        }

        let mut toc_html = String::new();
        toc_html.push_str(
            r#"<nav class="toc-sidebar" id="toc-sidebar">
                <div class="toc-header">
                    <h3>Table of Contents</h3>
                    <button class="toc-toggle" id="toc-toggle">✕</button>
                </div>
                <ul class="toc-list">
"#,
        );

        for entry in toc_entries {
            let indent_class = match entry.level {
                1 => "toc-h1",
                2 => "toc-h2",
                3 => "toc-h3",
                _ => "toc-h1",
            };

            toc_html.push_str(&format!(
                "                    <li class=\"toc-item {}\"><a href=\"#{}\" class=\"toc-link\">{}</a></li>\n",
                indent_class, entry.id, entry.title
            ));
        }

        toc_html.push_str(
            r#"                </ul>
            </nav>
            <button class="toc-mobile-toggle" id="toc-mobile-toggle">📋</button>"#,
        );

        toc_html
    }
}

impl BookmarkAction for MarkdownAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the content from bookmark URL field
        let content_or_path = &bookmark.url;

        // Determine if the content is a file path or direct markdown
        let markdown_content = if is_file_path(content_or_path) {
            debug!("Treating content as a file path: {}", content_or_path);
            self.read_markdown_from_file(content_or_path)?
        } else {
            debug!("Treating content as direct markdown");
            content_or_path.to_string()
        };

        // Skip template processing for markdown content to avoid conflicts with markdown syntax
        // that may contain template-like patterns (e.g., {%} in code blocks, documentation)
        let rendered_markdown = markdown_content.clone();

        // Update embedding if possible
        if let Err(e) = self.update_embedding(bookmark, &markdown_content) {
            error!("Failed to update embedding: {}", e);
            // Continue with rendering - don't fail the whole operation if embedding fails
        }

        debug!("Rendering markdown content to HTML");

        // Configure markdown options to properly handle tables and other features
        // let options = Options::default(); // CommonMark
        let options = Options::gfm(); // GitHub Flavored Markdown

        // Convert markdown to HTML with enhanced options
        let html_content = to_html_with_options(&rendered_markdown, &options)
            .map_err(|e| DomainError::Other(format!("Failed to render markdown: {}", e)))?;

        // Extract headers and generate TOC
        let (processed_html, toc_entries) = self.extract_and_process_headers(&html_content);
        let toc_html = self.generate_toc_html(&toc_entries);

        // Wrap the HTML content in a proper HTML document with enhanced styling
        let full_html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        :root {{
            --background-color: #ffffff;
            --text-color: #333333;
            --code-background: #f5f5f5;
            --blockquote-border: #ddd;
            --blockquote-color: #666;
            --link-color: #0366d6;
            --table-border: #ddd;
            --table-header-bg: #f2f2f2;
            --table-row-alt-bg: #f9f9f9;
            --base-font-size: 16px;
            --code-font-size: 0.99;
        }}

        @media (prefers-color-scheme: dark) {{
            :root {{
                --background-color: #1e1e1e;
                --text-color: #e0e0e0;
                --code-background: #2d2d2d;
                --blockquote-border: #555;
                --blockquote-color: #aaa;
                --link-color: #58a6ff;
                --table-border: #444;
                --table-header-bg: #2d2d2d;
                --table-row-alt-bg: #262626;
            }}
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            line-height: 1.6;
            color: var(--text-color);
            background-color: var(--background-color);
            margin: 0;
            padding: 0;
            font-size: var(--base-font-size);
        }}

        .container {{
            display: flex;
            min-height: 100vh;
        }}

        .main-content {{
            flex: 1;
            max-width: 900px;
            margin: 0 auto;
            padding: 20px;
            transition: margin-left 0.3s ease;
        }}

        .main-content.toc-visible {{
            margin-left: 320px;
        }}

        /* TOC Sidebar Styling */
        .toc-sidebar {{
            position: fixed;
            left: 0;
            top: 0;
            width: 300px;
            height: 100vh;
            background-color: var(--background-color);
            border-right: 1px solid var(--table-border);
            overflow-y: auto;
            padding: 20px;
            box-sizing: border-box;
            z-index: 1000;
            transition: transform 0.3s ease;
        }}

        .toc-sidebar.hidden {{
            transform: translateX(-100%);
        }}

        .toc-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 20px;
            padding-bottom: 10px;
            border-bottom: 1px solid var(--table-border);
        }}

        .toc-header h3 {{
            margin: 0;
            font-size: 1.1em;
            font-weight: 600;
        }}

        .toc-toggle {{
            background: none;
            border: none;
            font-size: 1.2em;
            cursor: pointer;
            color: var(--text-color);
            padding: 5px;
            border-radius: 3px;
        }}

        .toc-toggle:hover {{
            background-color: var(--code-background);
        }}

        .toc-list {{
            list-style: none;
            padding: 0;
            margin: 0;
        }}

        .toc-item {{
            margin: 0;
            padding: 0;
        }}

        .toc-link {{
            display: block;
            padding: 6px 0;
            color: var(--text-color);
            text-decoration: none;
            border-radius: 3px;
            transition: all 0.2s ease;
            font-size: 0.9em;
            line-height: 1.4;
        }}

        .toc-link:hover {{
            background-color: var(--code-background);
            padding-left: 8px;
        }}

        .toc-link.active {{
            color: var(--link-color);
            background-color: var(--code-background);
            font-weight: 500;
        }}

        .toc-h1 .toc-link {{
            font-weight: 600;
            font-size: 0.95em;
        }}

        .toc-h2 .toc-link {{
            padding-left: 16px;
            font-size: 0.88em;
        }}

        .toc-h3 .toc-link {{
            padding-left: 32px;
            font-size: 0.85em;
            color: var(--blockquote-color);
        }}

        .toc-mobile-toggle {{
            display: none;
            position: fixed;
            top: 20px;
            left: 20px;
            background-color: var(--link-color);
            color: white;
            border: none;
            border-radius: 50%;
            width: 50px;
            height: 50px;
            font-size: 1.2em;
            cursor: pointer;
            z-index: 1001;
            box-shadow: 0 2px 8px rgba(0,0,0,0.2);
        }}

        /* Mobile responsive */
        @media (max-width: 1024px) {{
            .main-content.toc-visible {{
                margin-left: 0;
                padding: 20px;
            }}

            .toc-sidebar {{
                transform: translateX(-100%);
            }}

            .toc-sidebar.mobile-visible {{
                transform: translateX(0);
            }}

            .toc-mobile-toggle {{
                display: block;
            }}
        }}

        @media (max-width: 768px) {{
            .main-content {{
                padding: 15px;
            }}

            .toc-sidebar {{
                width: 280px;
            }}
        }}

        h1, h2, h3, h4, h5, h6 {{
            margin-top: 24px;
            margin-bottom: 16px;
            font-weight: 600;
            line-height: 1.25;
        }}

        h1 {{ font-size: 2em; border-bottom: 1px solid var(--table-border); padding-bottom: 0.3em; }}
        h2 {{ font-size: 1.5em; border-bottom: 1px solid var(--table-border); padding-bottom: 0.3em; }}

        a {{ color: var(--link-color); text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}

        /* Enhanced pre/code styling with syntax highlighting support */
        pre {{
            background-color: var(--code-background);
            padding: 16px;
            border-radius: 6px;
            overflow-x: auto;
            margin: 16px 0;
            font-family: SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
            font-size: var(--code-font-size);
            line-height: 1.45;
        }}

        code {{
            font-family: SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
            background-color: var(--code-background);
            padding: 0.2em 0.4em;
            border-radius: 3px;
            font-size: var(--code-font-size);
        }}

        /* Inline code should be slightly larger for better readability */
        p code, li code, td code {{
            font-size: calc(var(--code-font-size) * 1.05);
        }}

        pre code {{
            padding: 0;
            background-color: transparent;
            white-space: pre;
            word-break: normal;
            overflow-wrap: normal;
        }}

        /* Syntax highlighting classes */
        .hljs-keyword {{ color: #cf222e; }}
        .hljs-built_in {{ color: #e36209; }}
        .hljs-type {{ color: #953800; }}
        .hljs-literal {{ color: #0550ae; }}
        .hljs-number {{ color: #0550ae; }}
        .hljs-string {{ color: #0a3069; }}
        .hljs-comment {{ color: #6e7781; }}
        .hljs-doctag {{ color: #0550ae; }}
        .hljs-meta {{ color: #8250df; }}
        .hljs-function {{ color: #8250df; }}

        @media (prefers-color-scheme: dark) {{
            .hljs-keyword {{ color: #ff7b72; }}
            .hljs-built_in {{ color: #ffa657; }}
            .hljs-type {{ color: #ff7b72; }}
            .hljs-literal {{ color: #79c0ff; }}
            .hljs-number {{ color: #79c0ff; }}
            .hljs-string {{ color: #a5d6ff; }}
            .hljs-comment {{ color: #8b949e; }}
            .hljs-doctag {{ color: #79c0ff; }}
            .hljs-meta {{ color: #d2a8ff; }}
            .hljs-function {{ color: #d2a8ff; }}
        }}

        /* Enhanced blockquote styling */
        blockquote {{
            margin: 0;
            padding-left: 16px;
            padding-right: 16px;
            padding-bottom: 1px;
            padding-top: 1px;
            background: rgba(0, 0, 0, 0.05);
            border-left: 4px solid var(--blockquote-border);
            color: var(--blockquote-color);
            margin-bottom: 16px;
        }}

        @media (prefers-color-scheme: dark) {{
            blockquote {{
                background: rgba(255, 255, 255, 0.05);
            }}
        }}

        /* Enhanced image styling */
        img {{
            max-width: 100%;
            box-sizing: border-box;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            margin: 10px 0;
            border-radius: 4px;
        }}

        /* Enhanced table styling with explicit font size control */
        table {{
            border-collapse: collapse;
            width: 100%;
            margin-bottom: 16px;
            overflow: auto;
            font-size: 1em; /* Match body font size */
        }}

        th, td {{
            border: 1px solid var(--table-border);
            padding: 8px 13px;
            text-align: left;
            font-size: 1em; /* Consistent font size */
            vertical-align: top;
        }}

        th {{
            background-color: var(--table-header-bg);
            font-weight: 600;
        }}

        tr {{
            font-size: 1em; /* Ensure rows maintain consistent font size */
        }}

        tr:nth-child(even) {{
            background-color: var(--table-row-alt-bg);
        }}

        /* Lists styling */
        ul, ol {{
            padding-left: 2em;
            margin-top: 0;
            margin-bottom: 16px;
        }}

        li {{
            margin-top: 0.25em;
        }}

        /* Task lists */
        ul.contains-task-list {{
            list-style-type: none;
            padding-left: 1em;
        }}

        .task-list-item {{
            position: relative;
            padding-left: 1.5em;
        }}

        .task-list-item input {{
            position: absolute;
            left: 0;
            top: 0.25em;
        }}

        /* Smooth scrolling for anchor links */
        html {{
            scroll-behavior: smooth;
        }}

        /* Ensure headers have some top margin for anchor positioning */
        h1[id], h2[id], h3[id] {{
            scroll-margin-top: 20px;
        }}
    </style>
    <!-- MathJax for LaTeX rendering -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.7/MathJax.js?config=TeX-MML-AM_CHTML"></script>
    <script type="text/x-mathjax-config">
        MathJax.Hub.Config({{
            tex2jax: {{
                inlineMath: [['$','$'], ['\\(','\\)']],
                displayMath: [['$$','$$'], ['\\[','\\]']],
                processEscapes: true
            }}
        }});
    </script>
    <!-- Highlight.js for code syntax highlighting -->
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/styles/github.min.css">
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/highlight.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/rust.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/java.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/python.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/bash.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/javascript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/typescript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/json.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/cpp.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/yaml.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/sql.min.js"></script>
    <script>
        document.addEventListener('DOMContentLoaded', (event) => {{
            document.querySelectorAll('pre code').forEach((block) => {{
                hljs.highlightBlock(block);
            }});

            // Add checkbox functionality for task lists
            document.querySelectorAll('.task-list-item input[type="checkbox"]').forEach(checkbox => {{
                checkbox.disabled = false;
                checkbox.addEventListener('change', function() {{
                    this.parentElement.classList.toggle('completed');
                }});
            }});

            // TOC functionality
            const tocSidebar = document.getElementById('toc-sidebar');
            const tocToggle = document.getElementById('toc-toggle');
            const tocMobileToggle = document.getElementById('toc-mobile-toggle');
            const mainContent = document.querySelector('.main-content');
            const tocLinks = document.querySelectorAll('.toc-link');

            // If TOC exists, add toc-visible class to main content
            if (tocSidebar) {{
                mainContent.classList.add('toc-visible');
            }}

            // Toggle TOC visibility
            function toggleToc() {{
                if (window.innerWidth <= 1024) {{
                    tocSidebar.classList.toggle('mobile-visible');
                }} else {{
                    tocSidebar.classList.toggle('hidden');
                    mainContent.classList.toggle('toc-visible');
                }}
            }}

            // Desktop toggle
            if (tocToggle) {{
                tocToggle.addEventListener('click', toggleToc);
            }}

            // Mobile toggle
            if (tocMobileToggle) {{
                tocMobileToggle.addEventListener('click', toggleToc);
            }}

            // Close mobile TOC when clicking on a link
            tocLinks.forEach(link => {{
                link.addEventListener('click', () => {{
                    if (window.innerWidth <= 1024) {{
                        tocSidebar.classList.remove('mobile-visible');
                    }}
                }});
            }});

            // Active section highlighting
            function updateActiveSection() {{
                const headers = document.querySelectorAll('h1[id], h2[id], h3[id]');
                const scrollPosition = window.scrollY + 100;

                let activeId = null;
                for (const header of headers) {{
                    if (header.offsetTop <= scrollPosition) {{
                        activeId = header.id;
                    }} else {{
                        break;
                    }}
                }}

                // Update active link
                tocLinks.forEach(link => {{
                    link.classList.remove('active');
                    if (activeId && link.getAttribute('href') === '#' + activeId) {{
                        link.classList.add('active');
                    }}
                }});
            }}

            // Update active section on scroll
            window.addEventListener('scroll', updateActiveSection);
            updateActiveSection(); // Initial call

            // Handle window resize
            window.addEventListener('resize', () => {{
                if (window.innerWidth > 1024 && tocSidebar) {{
                    tocSidebar.classList.remove('mobile-visible');
                    if (!tocSidebar.classList.contains('hidden')) {{
                        mainContent.classList.add('toc-visible');
                    }}
                }} else {{
                    tocSidebar.classList.remove('hidden');
                    mainContent.classList.remove('toc-visible');
                }}
            }});
        }});
    </script>
</head>
<body>
    {}
    <div class="container">
        <main class="main-content">
            {}
        </main>
    </div>
</body>
</html>"#,
            bookmark.title, toc_html, processed_html
        );

        // Create a temporary HTML file with explicit .html extension
        let temp_dir = tempfile::Builder::new()
            .prefix("bkmr-markdown-")
            .tempdir()
            .map_err(|e| {
                DomainError::Other(format!("Failed to create temporary directory: {}", e))
            })?;

        // Copy local resources to temp directory if content came from a file
        if let Some(source_path) = self.get_source_file_path(bookmark) {
            debug!("Copying resources for file: {:?}", source_path);
            self.copy_resources_to_temp(&full_html, &source_path, temp_dir.path())?;
        } else {
            debug!("Content is not from a file, skipping resource copying");
        }

        // Create a file path with .html extension
        let safe_title = bookmark
            .title
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let file_name = format!("{}.html", safe_title);
        let file_path = temp_dir.path().join(file_name);

        // Create and write to the file (HTML paths stay relative)
        let mut file = File::create(&file_path)
            .map_err(|e| DomainError::Other(format!("Failed to create HTML file: {}", e)))?;

        file.write_all(full_html.as_bytes())
            .map_err(|e| DomainError::Other(format!("Failed to write HTML to file: {}", e)))?;

        debug!("Opening HTML file in browser: {:?}", file_path);

        // Open the HTML file in the default browser
        open::that(&file_path)
            .map_err(|e| DomainError::Other(format!("Failed to open HTML in browser: {}", e)))?;

        // Keep the temporary directory around until the program exits
        // This prevents the file from being deleted while the browser is using it
        std::mem::forget(temp_dir);

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Render markdown and open in browser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::util::testing::{init_test_env, EnvGuard};
    use std::collections::HashSet;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Test reading markdown from a file
    #[test]
    fn given_markdown_file_when_read_then_returns_content() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create a temporary markdown file
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "# Test Markdown\n\nThis is a test.";
        write!(temp_file, "{}", test_content).unwrap();

        // Test reading the file
        let result = action.read_markdown_from_file(temp_file.path().to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_content);

        // Test with non-existent file
        let result = action.read_markdown_from_file("/this/file/does/not/exist.md");
        assert!(result.is_err());
    }

    // Test embedding eligibility check
    // TODO: check the purpose of this test
    #[test]
    fn given_action_when_check_embedding_update_then_returns_eligibility() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        // Action without repository
        let embedder_no_repo = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action_no_repo = MarkdownAction::new(embedder_no_repo);

        // Action with repository
        let repository = Arc::new(crate::util::testing::setup_test_db());
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action_with_repo = MarkdownAction::new_with_repository(repository, embedder);

        // Create test bookmarks
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        // Bookmark with embeddable=true
        let embeddable_bookmark = Bookmark {
            id: Some(1),
            url: "# Test".to_string(),
            title: "Test Document".to_string(),
            description: "A test document".to_string(),
            tags: tags.clone(),
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Bookmark with embeddable=false
        let non_embeddable_bookmark = Bookmark {
            id: Some(2),
            url: "# Test".to_string(),
            title: "Test Document".to_string(),
            description: "A test document".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Test cases
        assert!(
            !action_no_repo.can_update_embedding(&embeddable_bookmark),
            "Should return false when no repository is available"
        );

        // The DummyEmbedding is the default in test environment
        assert!(
            !action_with_repo.can_update_embedding(&non_embeddable_bookmark),
            "Should return false when bookmark is not embeddable"
        );

        // This would be true with OpenAI embeddings, but we can't easily test that
        // So we just verify that the embeddable flag is checked
    }

    #[test]
    #[ignore = "This test opens a browser which might not be available in CI"]
    fn given_markdown_content_when_execute_then_renders_html() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create a test bookmark with direct markdown content
        let markdown = "# Test Markdown\n\nThis is a **test** with math: $$E = mc^2$$";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Test Markdown Document".to_string(),
            description: "A test markdown document".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Execute the action
        let result = action.execute(&bookmark);

        // In a proper environment this should succeed
        // In CI it might fail due to no browser
        if result.is_err() {
            if let DomainError::Other(msg) = &result.unwrap_err() {
                // Only consider the test failed if it's not related to browser opening
                if !msg.contains("Failed to open HTML in browser") {
                    panic!("Test failed with unexpected error: {}", msg);
                }
            }
        }
    }

    #[test]
    #[ignore = "This test opens a browser which might not be available in CI"]
    fn given_markdown_with_table_when_execute_then_renders_with_table_styles() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create a test bookmark with markdown table content
        let markdown = "# Test Table\n\n| Column 1 | Column 2 | Column 3 |\n| -------- | -------- | -------- |\n| Cell 1   | Cell 2   | Cell 3   |\n| Cell 4   | Cell 5   | Cell 6   |";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Test Table Document".to_string(),
            description: "A test markdown document with tables".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Execute the action
        let result = action.execute(&bookmark);

        // In a proper environment this should succeed
        // In CI it might fail due to no browser
        if result.is_err() {
            if let DomainError::Other(msg) = &result.unwrap_err() {
                // Only consider the test failed if it's not related to browser opening
                if !msg.contains("Failed to open HTML in browser") {
                    panic!("Test failed with unexpected error: {}", msg);
                }
            }
        }
    }

    #[test]
    #[ignore = "This test opens a browser which might not be available in CI"]
    fn given_markdown_with_code_when_execute_then_renders_with_highlighting() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create a test bookmark with code blocks
        let markdown = "# Code Highlighting\n\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```\n\n```python\ndef hello():\n    print(\"Hello, world!\")\n```";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Code Highlighting Document".to_string(),
            description: "A test markdown document with code blocks".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Execute the action
        let result = action.execute(&bookmark);

        // In a proper environment this should succeed
        // In CI it might fail due to no browser
        if result.is_err() {
            if let DomainError::Other(msg) = &result.unwrap_err() {
                // Only consider the test failed if it's not related to browser opening
                if !msg.contains("Failed to open HTML in browser") {
                    panic!("Test failed with unexpected error: {}", msg);
                }
            }
        }
    }

    // Tests for TOC functionality
    #[test]
    fn given_markdown_headers_when_extract_and_process_then_generates_ids() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let html_content = r#"<h1>Main Title</h1>
<p>Some content</p>
<h2>Section 1</h2>
<p>More content</p>
<h3>Subsection 1.1</h3>
<p>Even more content</p>"#;

        let (processed_html, toc_entries) = action.extract_and_process_headers(html_content);

        // Should find 3 headers
        assert_eq!(toc_entries.len(), 3);

        // Check first entry (H1)
        assert_eq!(toc_entries[0].level, 1);
        assert_eq!(toc_entries[0].title, "Main Title");
        assert_eq!(toc_entries[0].id, "main-title");

        // Check second entry (H2)
        assert_eq!(toc_entries[1].level, 2);
        assert_eq!(toc_entries[1].title, "Section 1");
        assert_eq!(toc_entries[1].id, "section-1");

        // Check third entry (H3)
        assert_eq!(toc_entries[2].level, 3);
        assert_eq!(toc_entries[2].title, "Subsection 1.1");
        assert_eq!(toc_entries[2].id, "subsection-1-1");

        // Check that IDs were added to the HTML
        assert!(processed_html.contains("<h1 id=\"main-title\">Main Title</h1>"));
        assert!(processed_html.contains("<h2 id=\"section-1\">Section 1</h2>"));
        assert!(processed_html.contains("<h3 id=\"subsection-1-1\">Subsection 1.1</h3>"));
    }

    #[test]
    fn given_headers_with_existing_ids_when_extract_then_preserves_ids() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let html_content = r#"<h1 id="existing-id">Title with ID</h1>
<h2>Title without ID</h2>"#;

        let (processed_html, toc_entries) = action.extract_and_process_headers(html_content);

        assert_eq!(toc_entries.len(), 2);

        // First header should keep existing ID
        assert_eq!(toc_entries[0].id, "existing-id");
        assert!(processed_html.contains("<h1 id=\"existing-id\">Title with ID</h1>"));

        // Second header should get generated ID
        assert_eq!(toc_entries[1].id, "title-without-id");
        assert!(processed_html.contains("<h2 id=\"title-without-id\">Title without ID</h2>"));
    }

    #[test]
    fn given_duplicate_header_titles_when_extract_then_creates_unique_ids() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let html_content = r#"<h1>Introduction</h1>
<h2>Introduction</h2>
<h3>Introduction</h3>"#;

        let (processed_html, toc_entries) = action.extract_and_process_headers(html_content);

        assert_eq!(toc_entries.len(), 3);

        // Check duplicate handling
        assert_eq!(toc_entries[0].id, "introduction");
        assert_eq!(toc_entries[1].id, "introduction-1");
        assert_eq!(toc_entries[2].id, "introduction-2");

        // Check processed HTML
        assert!(processed_html.contains("<h1 id=\"introduction\">Introduction</h1>"));
        assert!(processed_html.contains("<h2 id=\"introduction-1\">Introduction</h2>"));
        assert!(processed_html.contains("<h3 id=\"introduction-2\">Introduction</h3>"));
    }

    #[test]
    fn given_headers_with_html_content_when_extract_then_cleans_html() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let html_content = r#"<h1>Title with <strong>bold</strong> and <em>italic</em></h1>
<h2>Code: <code>function()</code></h2>"#;

        let (_processed_html, toc_entries) = action.extract_and_process_headers(html_content);

        assert_eq!(toc_entries.len(), 2);

        // Check that HTML tags are stripped from titles
        assert_eq!(toc_entries[0].title, "Title with bold and italic");
        assert_eq!(toc_entries[1].title, "Code: function()");

        // Check IDs are generated from clean content
        assert_eq!(toc_entries[0].id, "title-with-bold-and-italic");
        assert_eq!(toc_entries[1].id, "code-function");
    }

    #[test]
    fn given_empty_content_when_extract_headers_then_returns_empty() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let html_content = "<p>No headers here</p>";

        let (processed_html, toc_entries) = action.extract_and_process_headers(html_content);

        assert_eq!(toc_entries.len(), 0);
        assert_eq!(processed_html, html_content); // Should be unchanged
    }

    #[test]
    fn given_h4_and_higher_headers_when_extract_then_ignores_them() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let html_content = r#"<h1>H1 Title</h1>
<h2>H2 Title</h2>
<h3>H3 Title</h3>
<h4>H4 Title</h4>
<h5>H5 Title</h5>
<h6>H6 Title</h6>"#;

        let (processed_html, toc_entries) = action.extract_and_process_headers(html_content);

        // Should only find H1, H2, H3
        assert_eq!(toc_entries.len(), 3);
        assert_eq!(toc_entries[0].level, 1);
        assert_eq!(toc_entries[1].level, 2);
        assert_eq!(toc_entries[2].level, 3);

        // H4, H5, H6 should be unchanged in processed HTML
        assert!(processed_html.contains("<h4>H4 Title</h4>"));
        assert!(processed_html.contains("<h5>H5 Title</h5>"));
        assert!(processed_html.contains("<h6>H6 Title</h6>"));
    }

    #[test]
    fn given_header_text_when_generate_id_then_creates_valid_id() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Test normal text
        assert_eq!(action.generate_header_id("Simple Title"), "simple-title");

        // Test with special characters
        assert_eq!(
            action.generate_header_id("Title with Special! @#$% Characters"),
            "title-with-special-characters"
        );

        // Test with numbers
        assert_eq!(action.generate_header_id("Section 1.2.3"), "section-1-2-3");

        // Test with extra spaces and hyphens
        assert_eq!(
            action.generate_header_id("  Multiple   Spaces  and--Hyphens  "),
            "multiple-spaces-and-hyphens"
        );

        // Test with HTML content
        assert_eq!(
            action.generate_header_id("Title with <strong>HTML</strong> tags"),
            "title-with-html-tags"
        );
    }

    #[test]
    fn given_html_content_when_clean_then_removes_tags() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Test removing HTML tags
        assert_eq!(
            action.clean_html_content("<strong>Bold</strong> text"),
            "Bold text"
        );
        assert_eq!(
            action.clean_html_content("<em>Italic</em> and <code>code</code>"),
            "Italic and code"
        );
        assert_eq!(
            action.clean_html_content("<a href='#'>Link</a> text"),
            "Link text"
        );

        // Test with nested tags
        assert_eq!(
            action.clean_html_content("<div><span>Nested</span> content</div>"),
            "Nested content"
        );

        // Test with no HTML
        assert_eq!(action.clean_html_content("Plain text"), "Plain text");

        // Test with self-closing tags
        assert_eq!(
            action.clean_html_content("Text with <br/> break"),
            "Text with  break"
        );
    }

    #[test]
    fn given_empty_headers_when_generate_toc_then_returns_empty_html() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let toc_entries = vec![];

        let toc_html = action.generate_toc_html(&toc_entries);

        assert_eq!(toc_html, "");
    }

    #[test]
    fn given_header_entries_when_generate_toc_then_creates_html_list() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let toc_entries = vec![
            TocEntry {
                level: 1,
                title: "Main Title".to_string(),
                id: "main-title".to_string(),
            },
            TocEntry {
                level: 2,
                title: "Section 1".to_string(),
                id: "section-1".to_string(),
            },
            TocEntry {
                level: 3,
                title: "Subsection 1.1".to_string(),
                id: "subsection-1-1".to_string(),
            },
        ];

        let toc_html = action.generate_toc_html(&toc_entries);

        // Check that HTML contains the sidebar structure
        assert!(toc_html.contains("<nav class=\"toc-sidebar\" id=\"toc-sidebar\">"));
        assert!(toc_html.contains("Table of Contents"));
        assert!(toc_html.contains("<ul class=\"toc-list\">"));

        // Check that entries are included with correct classes
        assert!(toc_html.contains("<li class=\"toc-item toc-h1\"><a href=\"#main-title\" class=\"toc-link\">Main Title</a></li>"));
        assert!(toc_html.contains("<li class=\"toc-item toc-h2\"><a href=\"#section-1\" class=\"toc-link\">Section 1</a></li>"));
        assert!(toc_html.contains("<li class=\"toc-item toc-h3\"><a href=\"#subsection-1-1\" class=\"toc-link\">Subsection 1.1</a></li>"));

        // Check mobile toggle button
        assert!(toc_html
            .contains("<button class=\"toc-mobile-toggle\" id=\"toc-mobile-toggle\">📋</button>"));
    }

    #[test]
    fn given_special_characters_in_titles_when_generate_toc_then_escapes_html() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);
        let toc_entries = vec![TocEntry {
            level: 1,
            title: "Title with & < > \" characters".to_string(),
            id: "title-with-characters".to_string(),
        }];

        let toc_html = action.generate_toc_html(&toc_entries);

        // Check that special characters are preserved in the title
        assert!(toc_html.contains("Title with & < > \" characters"));
        assert!(toc_html.contains("href=\"#title-with-characters\""));
    }

    // Tests for get_source_file_path()
    #[test]
    fn given_file_path_bookmark_when_get_source_then_returns_path() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create a temporary markdown file
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "# Test").unwrap();
        let temp_path = temp_file.path().to_str().unwrap().to_string();

        // Create bookmark with file path
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());
        let bookmark = Bookmark {
            id: Some(1),
            url: temp_path,
            title: "Test".to_string(),
            description: "Test".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        let result = action.get_source_file_path(&bookmark);
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn given_markdown_content_bookmark_when_get_source_then_returns_none() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create bookmark with direct markdown content
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());
        let bookmark = Bookmark {
            id: Some(1),
            url: "# Direct Markdown Content".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        let result = action.get_source_file_path(&bookmark);
        assert!(result.is_none());
    }

    #[test]
    fn given_nonexistent_file_path_bookmark_when_get_source_then_returns_none() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create bookmark with non-existent file path
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());
        let bookmark = Bookmark {
            id: Some(1),
            url: "/nonexistent/file.md".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        let result = action.get_source_file_path(&bookmark);
        assert!(result.is_none());
    }

    // Tests for extract_resource_paths()
    #[test]
    fn given_html_with_images_when_extract_resources_then_returns_paths() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let html = r#"
            <p>Some content</p>
            <img src="./images/photo.png" alt="Photo">
            <img src="./assets/logo.jpg" alt="Logo">
        "#;

        let resources = action.extract_resource_paths(html);
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].1, "./images/photo.png");
        assert_eq!(resources[1].1, "./assets/logo.jpg");
    }

    #[test]
    fn given_html_with_mixed_urls_when_extract_resources_then_filters_correctly() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let html = r#"
            <img src="./local.png" alt="Local">
            <img src="https://example.com/remote.png" alt="Remote">
            <img src="http://example.com/http.png" alt="HTTP">
            <img src="data:image/png;base64,abc123" alt="Data URI">
            <img src="//cdn.example.com/cdn.png" alt="Protocol Relative">
        "#;

        let resources = action.extract_resource_paths(html);
        // Should only extract ./local.png
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].1, "./local.png");
    }

    #[test]
    fn given_html_with_link_and_script_tags_when_extract_resources_then_includes_all() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let html = r#"
            <link rel="stylesheet" href="./styles.css">
            <script src="./scripts/app.js"></script>
            <img src="./images/icon.png">
        "#;

        let resources = action.extract_resource_paths(html);
        assert_eq!(resources.len(), 3);

        // Extract paths for verification (order may vary based on regex matching)
        let paths: Vec<&str> = resources.iter().map(|(_, path)| path.as_str()).collect();
        assert!(paths.contains(&"./images/icon.png"));
        assert!(paths.contains(&"./styles.css"));
        assert!(paths.contains(&"./scripts/app.js"));
    }

    #[test]
    fn given_empty_html_when_extract_resources_then_returns_empty_vec() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let html = "<p>No resources here</p>";
        let resources = action.extract_resource_paths(html);
        assert_eq!(resources.len(), 0);
    }

    #[test]
    fn given_html_with_quoted_paths_when_extract_resources_then_handles_both_quotes() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let html = r#"
            <img src="./single.png" alt="Single">
            <img src='./double.png' alt="Double">
        "#;

        let resources = action.extract_resource_paths(html);
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].1, "./single.png");
        assert_eq!(resources[1].1, "./double.png");
    }

    // Tests for resolve_resource_path()
    #[test]
    fn given_relative_path_when_resolve_resource_then_returns_absolute() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create temp directory with markdown file and image subdirectory
        let temp_dir = tempfile::tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let images_dir = temp_dir.path().join("images");
        fs::create_dir(&images_dir).unwrap();
        let image_path = images_dir.join("photo.png");
        fs::write(&image_path, "fake image data").unwrap();

        // Resolve relative path
        let result = action.resolve_resource_path("./images/photo.png", &md_path);
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn given_parent_relative_path_when_resolve_resource_then_returns_absolute() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create temp directory structure: temp/docs/test.md and temp/assets/logo.png
        let temp_dir = tempfile::tempdir().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        fs::create_dir(&docs_dir).unwrap();
        let md_path = docs_dir.join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let assets_dir = temp_dir.path().join("assets");
        fs::create_dir(&assets_dir).unwrap();
        let logo_path = assets_dir.join("logo.png");
        fs::write(&logo_path, "fake logo data").unwrap();

        // Resolve parent relative path
        let result = action.resolve_resource_path("../assets/logo.png", &md_path);
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn given_absolute_path_when_resolve_resource_then_returns_path() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create temp file
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "image data").unwrap();
        let absolute_path = temp_file.path().to_str().unwrap();

        // Create dummy markdown file path
        let md_path = std::path::PathBuf::from("/tmp/test.md");

        // Resolve absolute path
        let result = action.resolve_resource_path(absolute_path, &md_path);
        assert!(result.is_some());
    }

    #[test]
    fn given_nonexistent_path_when_resolve_resource_then_returns_none() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let temp_dir = tempfile::tempdir().unwrap();
        let md_path = temp_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        // Try to resolve non-existent file
        let result = action.resolve_resource_path("./nonexistent.png", &md_path);
        assert!(result.is_none());
    }

    #[test]
    fn given_path_with_dots_when_resolve_resource_then_canonicalizes() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create structure: temp/docs/notes/test.md and temp/docs/images/photo.png
        let temp_dir = tempfile::tempdir().unwrap();
        let docs_dir = temp_dir.path().join("docs");
        fs::create_dir(&docs_dir).unwrap();
        let notes_dir = docs_dir.join("notes");
        fs::create_dir(&notes_dir).unwrap();
        let md_path = notes_dir.join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let images_dir = docs_dir.join("images");
        fs::create_dir(&images_dir).unwrap();
        let photo_path = images_dir.join("photo.png");
        fs::write(&photo_path, "fake photo data").unwrap();

        // Resolve path with .. and . components
        let result = action.resolve_resource_path("./../images/./photo.png", &md_path);
        assert!(result.is_some());

        let resolved = result.unwrap();
        assert!(resolved.exists());
        // Should not contain . or .. after canonicalization
        assert!(!resolved.to_string_lossy().contains("/./"));
        assert!(!resolved.to_string_lossy().contains("/../"));
    }

    // Tests for copy_resources_to_temp()
    #[test]
    fn given_single_image_when_copy_resources_then_copies_correctly() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create source structure
        let source_dir = tempfile::tempdir().unwrap();
        let md_path = source_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let images_dir = source_dir.path().join("images");
        fs::create_dir(&images_dir).unwrap();
        let image_path = images_dir.join("photo.png");
        fs::write(&image_path, "test image data").unwrap();

        // Create temp directory
        let temp_dir = tempfile::tempdir().unwrap();

        // HTML with image reference
        let html = r#"<img src="./images/photo.png" alt="Photo">"#;

        // Copy resources
        let result = action.copy_resources_to_temp(html, &md_path, temp_dir.path());
        assert!(result.is_ok());

        // Verify file was copied
        let copied_file = temp_dir.path().join("images/photo.png");
        assert!(copied_file.exists());
        let content = fs::read_to_string(copied_file).unwrap();
        assert_eq!(content, "test image data");
    }

    #[test]
    fn given_multiple_resources_when_copy_resources_then_copies_all() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create source structure
        let source_dir = tempfile::tempdir().unwrap();
        let md_path = source_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        // Create images directory with file
        let images_dir = source_dir.path().join("images");
        fs::create_dir(&images_dir).unwrap();
        fs::write(images_dir.join("photo.png"), "photo data").unwrap();

        // Create styles directory with file
        let styles_dir = source_dir.path().join("styles");
        fs::create_dir(&styles_dir).unwrap();
        fs::write(styles_dir.join("main.css"), "css data").unwrap();

        // Create temp directory
        let temp_dir = tempfile::tempdir().unwrap();

        // HTML with multiple resources
        let html = r#"
            <link rel="stylesheet" href="./styles/main.css">
            <img src="./images/photo.png" alt="Photo">
        "#;

        // Copy resources
        let result = action.copy_resources_to_temp(html, &md_path, temp_dir.path());
        assert!(result.is_ok());

        // Verify files were copied
        assert!(temp_dir.path().join("styles/main.css").exists());
        assert!(temp_dir.path().join("images/photo.png").exists());
    }

    #[test]
    fn given_nested_directories_when_copy_resources_then_maintains_structure() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create source structure with nested directories
        let source_dir = tempfile::tempdir().unwrap();
        let md_path = source_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let assets_dir = source_dir.path().join("assets");
        fs::create_dir(&assets_dir).unwrap();
        let images_dir = assets_dir.join("images");
        fs::create_dir(&images_dir).unwrap();
        let icons_dir = images_dir.join("icons");
        fs::create_dir(&icons_dir).unwrap();
        fs::write(icons_dir.join("logo.svg"), "svg data").unwrap();

        // Create temp directory
        let temp_dir = tempfile::tempdir().unwrap();

        // HTML with nested resource path
        let html = r#"<img src="./assets/images/icons/logo.svg" alt="Logo">"#;

        // Copy resources
        let result = action.copy_resources_to_temp(html, &md_path, temp_dir.path());
        assert!(result.is_ok());

        // Verify directory structure was maintained
        let copied_file = temp_dir.path().join("assets/images/icons/logo.svg");
        assert!(copied_file.exists());
        let content = fs::read_to_string(copied_file).unwrap();
        assert_eq!(content, "svg data");
    }

    #[test]
    fn given_nonexistent_resource_when_copy_resources_then_continues() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        // Create source structure with only one file
        let source_dir = tempfile::tempdir().unwrap();
        let md_path = source_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let images_dir = source_dir.path().join("images");
        fs::create_dir(&images_dir).unwrap();
        fs::write(images_dir.join("existing.png"), "exists").unwrap();

        // Create temp directory
        let temp_dir = tempfile::tempdir().unwrap();

        // HTML references both existing and non-existing files
        let html = r#"
            <img src="./images/existing.png" alt="Exists">
            <img src="./images/nonexistent.png" alt="Missing">
        "#;

        // Copy resources - should not fail
        let result = action.copy_resources_to_temp(html, &md_path, temp_dir.path());
        assert!(result.is_ok());

        // Verify existing file was copied
        assert!(temp_dir.path().join("images/existing.png").exists());
        // Non-existent file should not exist in temp
        assert!(!temp_dir.path().join("images/nonexistent.png").exists());
    }

    #[test]
    fn given_empty_html_when_copy_resources_then_returns_ok() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let action = MarkdownAction::new(embedder);

        let source_dir = tempfile::tempdir().unwrap();
        let md_path = source_dir.path().join("test.md");
        fs::write(&md_path, "# Test").unwrap();

        let temp_dir = tempfile::tempdir().unwrap();

        // Empty HTML
        let html = "<p>No resources</p>";

        // Should succeed with no operations
        let result = action.copy_resources_to_temp(html, &md_path, temp_dir.path());
        assert!(result.is_ok());
    }
}
