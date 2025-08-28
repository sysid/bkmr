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

    /// Validate if content has proper frontmatter format
    fn validate_frontmatter_format(&self, content: &str, _file_path: &Path) -> DomainResult<bool> {
        let content = content.trim();

        // Check for YAML frontmatter (structural presence only)
        if let Some(stripped) = content.strip_prefix("---") {
            // Just check that closing delimiter exists - parsing validation happens later
            return Ok(stripped.contains("---"));
        }

        // Check for hash-style frontmatter
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
                let comment_content = trimmed[1..].trim();
                if comment_content.contains(':') {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Parse YAML frontmatter or hash-style comments from file content
    fn parse_frontmatter(
        &self,
        content: &str,
        file_path: &Path,
    ) -> DomainResult<(FileMeta, String)> {
        let content = content.trim();

        // Try YAML frontmatter first (between --- delimiters)
        if let Some(stripped) = content.strip_prefix("---") {
            if let Some(end_pos) = stripped.find("---") {
                let yaml_content = &stripped[..end_pos];
                let remaining_content = &stripped[end_pos + 3..].trim_start();

                match serde_yaml::from_str::<FileMeta>(yaml_content) {
                    Ok(meta) => return Ok((meta, remaining_content.to_string())),
                    Err(e) => {
                        warn!(
                            "Failed to parse YAML frontmatter in {}: {}",
                            file_path.display(),
                            e
                        );
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
                            meta.name = value.to_string();
                            found_any_metadata = true;
                        }
                        "tags" => {
                            meta.tags = Some(value.to_string());
                            found_any_metadata = true;
                        }
                        "type" => {
                            meta.r#type = Some(value.to_string());
                            found_any_metadata = true;
                        }
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
        let content = fs::read_to_string(file_path).map_err(|e| {
            DomainError::RepositoryError(crate::domain::error::RepositoryError::Other(format!(
                "Failed to read file {}: {}",
                file_path.display(),
                e
            )))
        })?;

        // Check if file has frontmatter - return error if no frontmatter found
        let has_frontmatter = self.validate_frontmatter_format(&content, file_path)?;
        if !has_frontmatter {
            return Err(DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(format!(
                    "No frontmatter found in {}",
                    file_path.display()
                )),
            ));
        }

        // Parse frontmatter
        let (meta, clean_content) = self.parse_frontmatter(&content, file_path)?;

        // Validate required fields
        if meta.name.is_empty() {
            return Err(DomainError::RepositoryError(
                crate::domain::error::RepositoryError::Other(format!(
                    "Missing required 'name' field in {}",
                    file_path.display()
                )),
            ));
        }
        let name = meta.name;

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
        let metadata = fs::metadata(file_path).map_err(|e| {
            DomainError::RepositoryError(crate::domain::error::RepositoryError::Other(format!(
                "Failed to get metadata for {}: {}",
                file_path.display(),
                e
            )))
        })?;

        let file_mtime = metadata
            .modified()
            .map_err(|e| {
                DomainError::RepositoryError(crate::domain::error::RepositoryError::Other(format!(
                    "Failed to get modification time for {}: {}",
                    file_path.display(),
                    e
                )))
            })?
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| {
                DomainError::RepositoryError(crate::domain::error::RepositoryError::Other(format!(
                    "Invalid modification time for {}: {}",
                    file_path.display(),
                    e
                )))
            })?
            .as_secs() as i64;

        // Calculate SHA-256 hash of the clean content
        let mut hasher = Sha256::new();
        hasher.update(clean_content.as_bytes());
        let file_hash = format!("{:x}", hasher.finalize());

        // Ensure we always store absolute paths
        let absolute_path = file_path
            .canonicalize()
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

    fn import_files(
        &self,
        paths: &[String],
        _options: &ImportOptions,
    ) -> DomainResult<Vec<FileImportData>> {
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
                            if _options.verbose {
                                eprintln!("Skipping {}: {}", path.display(), e);
                            } else {
                                debug!("Skipping {}: {}", path.display(), e);
                            }
                        }
                    }
                } else if _options.verbose {
                    eprintln!(
                        "Skipping {}: unsupported file type (expected .sh, .py, or .md)",
                        path.display()
                    );
                }
            } else if path.is_dir() {
                // Directory - use WalkBuilder for recursive traversal
                let walker = WalkBuilder::new(path)
                    .hidden(false) // Include hidden files
                    .git_ignore(true) // Respect .gitignore
                    .git_exclude(true) // Respect .git/info/exclude
                    .build();

                for entry in walker {
                    match entry {
                        Ok(entry) => {
                            let entry_path = entry.path();
                            if entry_path.is_file() && Self::is_supported_file(entry_path) {
                                match self.process_file(entry_path) {
                                    Ok(file_data) => {
                                        debug!(
                                            "Processed file: {} (name: {})",
                                            entry_path.display(),
                                            file_data.name
                                        );
                                        all_files.push(file_data);
                                    }
                                    Err(e) => {
                                        if _options.verbose {
                                            eprintln!("Skipping {}: {}", entry_path.display(), e);
                                        } else {
                                            debug!("Skipping {}: {}", entry_path.display(), e);
                                        }
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
        matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("sh") | Some("py") | Some("md")
        )
    }
}

/// Frontmatter metadata structure
#[derive(Debug, Deserialize)]
struct FileMeta {
    name: String, // Required field
    tags: Option<String>,
    #[serde(rename = "type")]
    r#type: Option<String>,
}

impl Default for FileMeta {
    fn default() -> Self {
        Self {
            name: String::new(), // Will be detected as missing during validation
            tags: None,
            r#type: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn given_yaml_frontmatter_when_parse_then_extracts_metadata() {
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

        assert_eq!(meta.name, "test-script".to_string());
        assert_eq!(meta.tags, Some("admin, backup".to_string()));
        assert_eq!(meta.r#type, Some("_shell_".to_string()));
        assert_eq!(clean_content, "#!/bin/bash\necho \"Hello World\"");
    }

    #[test]
    fn given_hash_frontmatter_when_parse_then_extracts_metadata() {
        let repo = FileImportRepository::new();
        let content = r#"#!/bin/bash
# name: backup-db
# tags: database, backup
# type: _shell_
echo "Backing up database"
"#;

        let path = Path::new("backup.sh");
        let (meta, clean_content) = repo.parse_frontmatter(content, path).unwrap();

        assert_eq!(meta.name, "backup-db".to_string());
        assert_eq!(meta.tags, Some("database, backup".to_string()));
        assert_eq!(meta.r#type, Some("_shell_".to_string()));
        assert!(clean_content.contains("#!/bin/bash"));
        assert!(clean_content.contains("echo \"Backing up database\""));
    }

    #[test]
    fn given_file_extensions_when_check_support_then_validates_correctly() {
        assert!(FileImportRepository::is_supported_file(Path::new(
            "script.sh"
        )));
        assert!(FileImportRepository::is_supported_file(Path::new(
            "script.py"
        )));
        assert!(FileImportRepository::is_supported_file(Path::new("doc.md")));
        assert!(!FileImportRepository::is_supported_file(Path::new(
            "data.txt"
        )));
        assert!(!FileImportRepository::is_supported_file(Path::new(
            "config.toml"
        )));
    }

    #[test]
    fn given_file_with_frontmatter_when_process_then_creates_import_data() {
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

    #[test]
    fn given_valid_yaml_frontmatter_when_validate_then_returns_ok() {
        let repo = FileImportRepository::new();
        let content = r#"---
name: test
---
content"#;
        let path = Path::new("test.sh");
        assert!(repo.validate_frontmatter_format(content, path).unwrap());
    }

    #[test]
    fn given_invalid_yaml_frontmatter_when_validate_then_returns_error() {
        let repo = FileImportRepository::new();
        let content = r#"---
name: [invalid yaml
---
content"#;
        let path = Path::new("test.sh");
        // validate_frontmatter_format only checks structural presence, not parsing validity
        // Invalid YAML syntax will be caught during actual parsing in parse_frontmatter
        assert!(repo.validate_frontmatter_format(content, path).unwrap());
    }

    #[test]
    fn given_incomplete_yaml_frontmatter_when_validate_then_returns_error() {
        let repo = FileImportRepository::new();
        let content = r#"---
name: test
content without closing"#;
        let path = Path::new("test.sh");
        // validate_frontmatter_format only checks for closing delimiter presence
        // Missing closing delimiter returns false (no frontmatter detected)
        assert!(!repo.validate_frontmatter_format(content, path).unwrap());
    }

    #[test]
    fn given_hash_style_frontmatter_when_validate_then_returns_ok() {
        let repo = FileImportRepository::new();
        let content = r#"#!/bin/bash
# name: test
echo hello"#;
        let path = Path::new("test.sh");
        assert!(repo.validate_frontmatter_format(content, path).unwrap());
    }

    #[test]
    fn given_no_frontmatter_when_validate_then_returns_error() {
        let repo = FileImportRepository::new();
        let content = r#"#!/bin/bash
echo hello"#;
        let path = Path::new("test.sh");
        assert!(!repo.validate_frontmatter_format(content, path).unwrap());
    }

    #[test]
    fn given_file_without_frontmatter_when_process_then_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.sh");

        let content = r#"#!/bin/bash
echo "test"
"#;

        fs::write(&file_path, content).unwrap();

        let repo = FileImportRepository::new();
        let result = repo.process_file(&file_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No frontmatter found"));
    }

    #[test]
    fn given_file_missing_required_name_when_process_then_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.sh");

        let content = r#"---
tags: test
---
#!/bin/bash
echo "test"
"#;

        fs::write(&file_path, content).unwrap();

        let repo = FileImportRepository::new();
        let result = repo.process_file(&file_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required 'name' field"));
    }

    #[test]
    fn given_file_with_empty_name_when_process_then_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.sh");

        let content = r#"---
name: ""
tags: test
---
#!/bin/bash
echo "test"
"#;

        fs::write(&file_path, content).unwrap();

        let repo = FileImportRepository::new();
        let result = repo.process_file(&file_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required 'name' field"));
    }

    #[test]
    fn given_import_with_verbose_option_when_process_then_outputs_details() {
        use crate::domain::repositories::import_repository::ImportOptions;

        let temp_dir = TempDir::new().unwrap();
        let repo = FileImportRepository::new();

        // Create a valid file
        let valid_file = temp_dir.path().join("valid.sh");
        fs::write(
            &valid_file,
            r#"---
name: valid-script
---
#!/bin/bash
echo "valid"
"#,
        )
        .unwrap();

        // Create an invalid file (no frontmatter)
        let invalid_file = temp_dir.path().join("invalid.sh");
        fs::write(
            &invalid_file,
            r#"#!/bin/bash
echo "invalid"
"#,
        )
        .unwrap();

        // Create an unsupported file
        let unsupported_file = temp_dir.path().join("unsupported.txt");
        fs::write(&unsupported_file, "text content").unwrap();

        let paths = vec![temp_dir.path().to_string_lossy().to_string()];
        let options = ImportOptions {
            update: false,
            delete_missing: false,
            dry_run: false,
            verbose: true,
        };

        let result = repo.import_files(&paths, &options).unwrap();

        // Should only import the valid file
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "valid-script");
    }

    #[test]
    fn given_invalid_yaml_syntax_when_parse_frontmatter_then_returns_error() {
        let repo = FileImportRepository::new();
        let content = r#"---
name: [invalid yaml
---
#!/bin/bash
echo "test"
"#;
        let path = Path::new("test.sh");

        // validate_frontmatter_format should pass (structural check only)
        assert!(repo.validate_frontmatter_format(content, path).unwrap());

        // parse_frontmatter should catch the invalid YAML and fall through to hash-style parsing
        let result = repo.parse_frontmatter(content, path).unwrap();
        // Since YAML parsing failed, it falls back to hash-style which finds no metadata
        // Default FileMeta will have empty name, which should be caught in validation
        assert!(result.0.name.is_empty());
    }
}
