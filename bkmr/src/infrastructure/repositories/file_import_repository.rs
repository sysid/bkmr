// src/infrastructure/repositories/file_import_repository.rs

use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::import_repository::{
    BookmarkImportData, FileImportData, ImportOptions, ImportRepository,
};
use crate::domain::tag::Tag;
use crate::infrastructure::repositories::json_import_repository::JsonImportRepository;
use ignore::WalkBuilder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct FileImportRepository {
    json_import_repository: JsonImportRepository,
}

impl FileImportRepository {
    pub fn new() -> Self {
        Self {
            json_import_repository: JsonImportRepository::new(),
        }
    }

    /// Parse YAML frontmatter or hash-style comments from file content
    fn parse_frontmatter(&self, content: &str, file_path: &Path) -> DomainResult<(FileMeta, String)> {
        let content = content.trim();
        
        // Try YAML frontmatter first (between --- delimiters)
        if content.starts_with("---") {
            if let Some(end_pos) = content[3..].find("---") {
                let yaml_content = &content[3..end_pos + 3];
                let remaining_content = &content[end_pos + 6..].trim_start();
                
                match serde_yaml::from_str::<FileMeta>(yaml_content) {
                    Ok(meta) => return Ok((meta, remaining_content.to_string())),
                    Err(e) => {
                        warn!("Failed to parse YAML frontmatter in {}: {}", file_path.display(), e);
                        // Fall through to try hash-style comments
                    }
                }
            }
        }

        // Try hash-style comments (# key: value)
        let mut meta = FileMeta::default();
        let mut content_lines = Vec::new();
        let mut in_frontmatter = true;
        let mut found_any_metadata = false;

        for line in content.lines() {
            let trimmed = line.trim();
            
            if in_frontmatter && trimmed.starts_with('#') {
                let comment_content = trimmed[1..].trim();
                
                if let Some((key, value)) = comment_content.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    
                    match key {
                        "name" => {
                            meta.name = Some(value.to_string());
                            found_any_metadata = true;
                        },
                        "tags" => {
                            meta.tags = Some(value.to_string());
                            found_any_metadata = true;
                        },
                        "type" => {
                            meta.r#type = Some(value.to_string());
                            found_any_metadata = true;
                        },
                        _ => {
                            // Unknown frontmatter key, treat as regular comment
                            content_lines.push(line);
                        }
                    }
                } else {
                    // Regular comment without key:value, include in content
                    content_lines.push(line);
                    // Only end frontmatter if we've found some metadata and hit a non-metadata comment
                    if found_any_metadata {
                        in_frontmatter = false;
                    }
                }
            } else if in_frontmatter && (trimmed.is_empty() || trimmed.starts_with("#!")) {
                // Allow empty lines and shebang in frontmatter section
                content_lines.push(line);
            } else {
                // Non-comment line that's not shebang - end frontmatter
                in_frontmatter = false;
                content_lines.push(line);
            }
        }

        let remaining_content = content_lines.join("\n");
        Ok((meta, remaining_content))
    }

    /// Process a single file and extract its metadata
    pub fn process_file(&self, file_path: &Path) -> DomainResult<FileImportData> {
        debug!("Processing file: {}", file_path.display());

        // Read file content
        let content = fs::read_to_string(file_path)
            .map_err(|e| DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(
                    format!("Failed to read file {}: {}", file_path.display(), e)
                )
            ))?;

        // Parse frontmatter
        let (meta, clean_content) = self.parse_frontmatter(&content, file_path)?;

        // Validate required fields
        let name = meta.name.ok_or_else(|| {
            DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(
                    format!("Missing required 'name' field in {}", file_path.display())
                )
            )
        })?;

        // Parse tags
        let tags = if let Some(tag_str) = meta.tags {
            Tag::parse_tags(&tag_str)?
        } else {
            HashSet::new()
        };

        // Determine content type based on file extension and frontmatter
        let content_type = meta.r#type.unwrap_or_else(|| {
            match file_path.extension().and_then(|s| s.to_str()) {
                Some("md") => "_md_".to_string(),
                Some("py") => "_shell_".to_string(),
                Some("sh") => "_shell_".to_string(),
                _ => "_shell_".to_string(), // Default
            }
        });

        // Get file metadata
        let metadata = fs::metadata(file_path)
            .map_err(|e| DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(
                    format!("Failed to get metadata for {}: {}", file_path.display(), e)
                )
            ))?;

        let file_mtime = metadata
            .modified()
            .map_err(|e| DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(
                    format!("Failed to get modification time for {}: {}", file_path.display(), e)
                )
            ))?
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(
                    format!("Invalid modification time for {}: {}", file_path.display(), e)
                )
            ))?
            .as_secs() as i64;

        // Calculate SHA-256 hash of the clean content
        let mut hasher = Sha256::new();
        hasher.update(clean_content.as_bytes());
        let file_hash = format!("{:x}", hasher.finalize());

        // Ensure we always store absolute paths
        let absolute_path = file_path.canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf());

        Ok(FileImportData {
            name,
            tags,
            content_type,
            content: clean_content,
            file_path: absolute_path,
            file_mtime,
            file_hash,
        })
    }
}

impl ImportRepository for FileImportRepository {
    fn import_json_bookmarks(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>> {
        // Delegate to JsonImportRepository
        self.json_import_repository.import_json_bookmarks(path)
    }

    fn import_text_documents(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>> {
        // Delegate to JsonImportRepository
        self.json_import_repository.import_text_documents(path)
    }

    fn import_files(&self, paths: &[String], _options: &ImportOptions) -> DomainResult<Vec<FileImportData>> {
        info!("Starting file import from {} paths", paths.len());
        let mut all_files = Vec::new();

        for path_str in paths {
            let path = Path::new(path_str);
            
            if !path.exists() {
                warn!("Path does not exist: {}", path.display());
                continue;
            }

            if path.is_file() {
                // Single file
                if Self::is_supported_file(path) {
                    match self.process_file(path) {
                        Ok(file_data) => all_files.push(file_data),
                        Err(e) => {
                            warn!("Failed to process file {}: {}", path.display(), e);
                        }
                    }
                }
            } else if path.is_dir() {
                // Directory - use WalkBuilder for recursive traversal
                let walker = WalkBuilder::new(path)
                    .hidden(false)        // Include hidden files
                    .git_ignore(true)     // Respect .gitignore
                    .git_exclude(true)    // Respect .git/info/exclude
                    .build();

                for entry in walker {
                    match entry {
                        Ok(entry) => {
                            let entry_path = entry.path();
                            if entry_path.is_file() && Self::is_supported_file(entry_path) {
                                match self.process_file(entry_path) {
                                    Ok(file_data) => {
                                        debug!("Processed file: {} (name: {})", entry_path.display(), file_data.name);
                                        all_files.push(file_data);
                                    }
                                    Err(e) => {
                                        warn!("Failed to process file {}: {}", entry_path.display(), e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Error walking directory: {}", e);
                        }
                    }
                }
            }
        }

        info!("Found {} files to import", all_files.len());
        Ok(all_files)
    }
}

impl FileImportRepository {
    /// Check if file has supported extension
    fn is_supported_file(path: &Path) -> bool {
        match path.extension().and_then(|s| s.to_str()) {
            Some("sh") | Some("py") | Some("md") => true,
            _ => false,
        }
    }
}

/// Frontmatter metadata structure
#[derive(Debug, Deserialize, Default)]
struct FileMeta {
    name: Option<String>,
    tags: Option<String>,
    #[serde(rename = "type")]
    r#type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_yaml_frontmatter() {
        let repo = FileImportRepository::new();
        let content = r#"---
name: test-script
tags: admin, backup
type: _shell_
---
#!/bin/bash
echo "Hello World"
"#;
        
        let path = Path::new("test.sh");
        let (meta, clean_content) = repo.parse_frontmatter(content, path).unwrap();
        
        assert_eq!(meta.name, Some("test-script".to_string()));
        assert_eq!(meta.tags, Some("admin, backup".to_string()));
        assert_eq!(meta.r#type, Some("_shell_".to_string()));
        assert_eq!(clean_content, "#!/bin/bash\necho \"Hello World\"");
    }

    #[test]
    fn test_parse_hash_frontmatter() {
        let repo = FileImportRepository::new();
        let content = r#"#!/bin/bash
# name: backup-db
# tags: database, backup
# type: _shell_
echo "Backing up database"
"#;
        
        let path = Path::new("backup.sh");
        let (meta, clean_content) = repo.parse_frontmatter(content, path).unwrap();
        
        assert_eq!(meta.name, Some("backup-db".to_string()));
        assert_eq!(meta.tags, Some("database, backup".to_string()));
        assert_eq!(meta.r#type, Some("_shell_".to_string()));
        assert!(clean_content.contains("#!/bin/bash"));
        assert!(clean_content.contains("echo \"Backing up database\""));
    }

    #[test]
    fn test_is_supported_file() {
        assert!(FileImportRepository::is_supported_file(Path::new("script.sh")));
        assert!(FileImportRepository::is_supported_file(Path::new("script.py")));
        assert!(FileImportRepository::is_supported_file(Path::new("doc.md")));
        assert!(!FileImportRepository::is_supported_file(Path::new("data.txt")));
        assert!(!FileImportRepository::is_supported_file(Path::new("config.toml")));
    }

    #[test]
    fn test_process_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.sh");
        
        let content = r#"---
name: test-script
tags: test, demo
---
#!/bin/bash
echo "test"
"#;
        
        fs::write(&file_path, content).unwrap();
        
        let repo = FileImportRepository::new();
        let result = repo.process_file(&file_path).unwrap();
        
        assert_eq!(result.name, "test-script");
        assert_eq!(result.content_type, "_shell_");
        assert_eq!(result.content, "#!/bin/bash\necho \"test\"");
        assert!(result.file_hash.len() == 64); // SHA-256 hex string
        assert!(result.file_mtime > 0);
    }
}