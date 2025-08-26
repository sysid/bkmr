// src/application/services/bookmark_service_impl.rs
use std::collections::HashSet;
use std::sync::Arc;

use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::services::bookmark_service::BookmarkService;
use crate::domain::bookmark::{Bookmark, BookmarkBuilder};
use crate::domain::embedding::{serialize_embedding, Embedder};
use crate::domain::repositories::import_repository::{
    BookmarkImportData, FileImportData, ImportRepository,
};
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::search::{SemanticSearch, SemanticSearchResult};
use crate::domain::tag::Tag;
use crate::infrastructure::http;
use crate::util::helper::calc_content_hash;
use crate::util::validation::ValidationHelper;
use std::path::Path;
use tracing::{debug, instrument, warn};

#[derive(Debug)]
pub struct BookmarkServiceImpl<R: BookmarkRepository> {
    repository: Arc<R>,
    embedder: Arc<dyn Embedder>,
    import_repository: Arc<dyn ImportRepository>,
}

impl<R: BookmarkRepository> BookmarkServiceImpl<R> {
    pub fn new(
        repository: Arc<R>,
        embedder: Arc<dyn Embedder>,
        import_repository: Arc<dyn ImportRepository>,
    ) -> Self {
        Self {
            repository,
            embedder,
            import_repository,
        }
    }
}

impl<R: BookmarkRepository> BookmarkService for BookmarkServiceImpl<R> {
    #[instrument(skip(self, tags), level = "debug",
               fields(url = %url, title = %title.unwrap_or("None"), fetch_metadata = %fetch_metadata))]
    fn add_bookmark(
        &self,
        url: &str,
        title: Option<&str>,
        description: Option<&str>,
        tags: Option<&HashSet<Tag>>,
        fetch_metadata: bool,
    ) -> ApplicationResult<Bookmark> {
        // Check if bookmark with URL already exists
        let existing_id = self.repository.exists_by_url(url)?;
        if existing_id != -1 {
            return Err(ApplicationError::BookmarkExists(
                existing_id,
                url.to_string(),
            ));
        }

        let (title_str, desc_str, _keywords) =
            if fetch_metadata && (url.starts_with("http://") || url.starts_with("https://")) {
                // Try to fetch metadata from web URLs
                match http::load_url_details(url) {
                    Ok((t, d, k)) => (
                        title.map_or(t, |user_title| user_title.to_string()),
                        description.map_or(d, |user_desc| user_desc.to_string()),
                        k,
                    ),
                    Err(e) => {
                        debug!("Failed to fetch URL metadata: {}", e);
                        (
                            title.map_or_else(|| "Untitled".to_string(), |t| t.to_string()),
                            description.map_or_else(String::new, ToString::to_string),
                            String::new(),
                        )
                    }
                }
            } else {
                // Use provided or default values for non-web URLs or when fetching is disabled
                (
                    title.map_or_else(|| "Untitled".to_string(), ToString::to_string),
                    description.map_or_else(String::new, ToString::to_string),
                    String::new(),
                )
            };

        let all_tags = tags.cloned().unwrap_or_default();

        // Create and save bookmark
        debug!(
            "Creating bookmark: '{}' with {} tags",
            title_str,
            all_tags.len()
        );
        let mut bookmark =
            Bookmark::new(url, &title_str, &desc_str, all_tags, self.embedder.as_ref())?;

        self.repository.add(&mut bookmark)?;

        Ok(bookmark)
    }

    #[instrument(skip(self), level = "debug")]
    fn delete_bookmark(&self, id: i32) -> ApplicationResult<bool> {
        ValidationHelper::validate_bookmark_id(id)?;

        let result = self.repository.delete(id)?;
        Ok(result)
    }

    #[instrument(skip(self), level = "debug")]
    fn get_bookmark(&self, id: i32) -> ApplicationResult<Option<Bookmark>> {
        ValidationHelper::validate_bookmark_id(id)?;

        let bookmark = self.repository.get_by_id(id)?;
        Ok(bookmark)
    }

    #[instrument(skip(self), level = "debug")]
    fn set_bookmark_embeddable(&self, id: i32, embeddable: bool) -> ApplicationResult<Bookmark> {
        let mut bookmark = ValidationHelper::validate_and_get_bookmark(id, &*self.repository)?;

        bookmark.set_embeddable(embeddable);

        // If embeddable is being turned off, explicitly clear the embeddings
        if !embeddable {
            debug!("Setting bookmark {} to non-embeddable", id);
            bookmark.embedding = None;
            bookmark.content_hash = None;

            // No need to force embedding creation since we're turning it off
            self.update_bookmark(bookmark, false)
        } else {
            // If embeddable is being turned on, force the creation of embeddings
            self.update_bookmark(bookmark, true)
        }
    }

    // Note: This embedding logic could be moved to a dedicated domain service
    // to better separate concerns between bookmark persistence and embedding computation
    #[instrument(skip(self), level = "debug")]
    fn update_bookmark(
        &self,
        mut bookmark: Bookmark,
        force_embedding: bool,
    ) -> ApplicationResult<Bookmark> {
        ValidationHelper::validate_bookmark_id(bookmark.id.ok_or_else(|| {
            ApplicationError::Validation("Bookmark ID is required for update".to_string())
        })?)?;

        let content = bookmark.get_content_for_embedding();
        let new_hash = calc_content_hash(&content);

        // Only update embedding if embeddable flag is true
        if bookmark.embeddable {
            // Generate new embedding if forced or content has changed
            if force_embedding || bookmark.content_hash.as_ref() != Some(&new_hash) {
                debug!(
                    "Generating new embedding (force={}, content_changed={})",
                    force_embedding,
                    bookmark.content_hash.as_ref() != Some(&new_hash)
                );

                // Generate new embedding
                if let Ok(Some(embedding_vector)) = self.embedder.embed(&content) {
                    if let Ok(serialized) = serialize_embedding(embedding_vector) {
                        bookmark.embedding = Some(serialized);
                        bookmark.content_hash = Some(new_hash);
                    }
                }
            } else {
                debug!("Skipping embedding generation - content unchanged and not forced");
            }
        } else {
            // Clear embedding if not embeddable
            bookmark.embedding = None;
            bookmark.content_hash = None;
        }

        bookmark.record_access();
        self.repository.update(&bookmark)?;
        Ok(bookmark)
    }

    #[instrument(skip(self, tags), level = "debug")]
    fn add_tags_to_bookmark(&self, id: i32, tags: &HashSet<Tag>) -> ApplicationResult<Bookmark> {
        let mut bookmark = ValidationHelper::validate_and_get_bookmark(id, &*self.repository)?;

        for tag in tags {
            bookmark.add_tag(tag.clone())?;
        }
        self.update_bookmark(bookmark, false)
    }

    #[instrument(skip(self, tags), level = "debug")]
    fn remove_tags_from_bookmark(
        &self,
        id: i32,
        tags: &HashSet<Tag>,
    ) -> ApplicationResult<Bookmark> {
        let mut bookmark = ValidationHelper::validate_and_get_bookmark(id, &*self.repository)?;

        for tag in tags {
            let _ = bookmark.remove_tag(tag);
        }
        self.update_bookmark(bookmark, false)
    }

    #[instrument(skip(self, tags), level = "debug")]
    fn replace_bookmark_tags(&self, id: i32, tags: &HashSet<Tag>) -> ApplicationResult<Bookmark> {
        let mut bookmark = ValidationHelper::validate_and_get_bookmark(id, &*self.repository)?;

        bookmark.set_tags(tags.clone())?;
        self.update_bookmark(bookmark, false)
    }

    #[instrument(skip_all, level = "debug")]
    fn search_bookmarks(&self, query: &BookmarkQuery) -> ApplicationResult<Vec<Bookmark>> {
        debug!("Searching bookmarks with query: {:?}", query);

        let bookmarks = self.repository.search(query)?;
        Ok(bookmarks)
    }

    // Implement the convenience method for text search
    #[instrument(skip_all, level = "debug")]
    fn search_bookmarks_by_text(&self, query: &str) -> ApplicationResult<Vec<Bookmark>> {
        let query = BookmarkQuery::new()
            .with_text_query(Some(query))
            .with_sort_by_date(SortDirection::Descending);

        self.search_bookmarks(&query)
    }

    #[instrument(skip(self, search), level = "debug")]
    fn semantic_search(
        &self,
        search: &SemanticSearch,
    ) -> ApplicationResult<Vec<SemanticSearchResult>> {
        let bookmarks = self.repository.get_all()?;
        search
            .execute(&bookmarks, self.embedder.as_ref())
            .map_err(ApplicationError::from)
    }

    #[instrument(skip(self), level = "debug")]
    fn get_bookmark_by_url(&self, url: &str) -> ApplicationResult<Option<Bookmark>> {
        let bookmark = self.repository.get_by_url(url)?;
        Ok(bookmark)
    }

    #[instrument(skip(self), level = "debug")]
    fn get_all_bookmarks(
        &self,
        sort_direction: Option<SortDirection>,
        limit: Option<usize>,
    ) -> ApplicationResult<Vec<Bookmark>> {
        let bookmarks = match sort_direction {
            Some(direction) => self.repository.get_by_access_date(direction, limit)?,
            None => {
                let mut query = BookmarkQuery::new();
                if let Some(limit_val) = limit {
                    query = query.with_limit(Some(limit_val));
                }
                self.repository.search(&query)?
            }
        };

        Ok(bookmarks)
    }

    #[instrument(skip(self), level = "debug")]
    fn get_random_bookmarks(&self, count: usize) -> ApplicationResult<Vec<Bookmark>> {
        let bookmarks = self.repository.get_random(count)?;
        Ok(bookmarks)
    }

    #[instrument(skip(self), level = "debug")]
    fn get_bookmarks_for_forced_backfill(&self) -> ApplicationResult<Vec<Bookmark>> {
        let all_bookmarks = self.repository.get_all()?;

        // Filter to only embeddable bookmarks that don't have the _imported_ tag
        let filtered_bookmarks = all_bookmarks
            .into_iter()
            .filter(|bookmark| {
                bookmark.embeddable && !bookmark.tags.iter().any(|tag| tag.value() == "_imported_")
            })
            .collect();

        Ok(filtered_bookmarks)
    }

    #[instrument(skip(self), level = "debug")]
    fn get_bookmarks_without_embeddings(&self) -> ApplicationResult<Vec<Bookmark>> {
        // Use the repository method to get only embeddable bookmarks without embeddings
        let bookmarks = self.repository.get_embeddable_without_embeddings()?;
        Ok(bookmarks)
    }

    #[instrument(skip(self), level = "debug")]
    fn record_bookmark_access(&self, id: i32) -> ApplicationResult<Bookmark> {
        let mut bookmark = ValidationHelper::validate_and_get_bookmark(id, &*self.repository)?;

        bookmark.record_access();

        self.repository.update(&bookmark)?;

        Ok(bookmark)
    }

    #[instrument(skip(self), level = "debug")]
    fn load_json_bookmarks(&self, path: &str, dry_run: bool) -> ApplicationResult<usize> {
        let imports = self
            .import_repository
            .import_json_bookmarks(path)
            .map_err(|e| ApplicationError::Other(format!("Failed to import data: {}", e)))?;

        if dry_run {
            return Ok(imports.len());
        }

        let mut processed_count = 0;

        for import in imports {
            // Check if bookmark with URL already exists
            let existing_id = self.repository.exists_by_url(&import.url)?;
            if existing_id != -1 {
                debug!(
                    "Bookmark with URL {} already exists (ID: {}), skipping",
                    import.url, existing_id
                );
                continue;
            }

            debug!("Processing import: {}", import.url);

            // Create the bookmark
            let mut bookmark = Bookmark::new(
                &import.url,
                &import.title,
                &import.content,
                import.tags,
                self.embedder.as_ref(),
            )?;
            // todo: embeddings and code duplication

            self.repository.add(&mut bookmark)?;
            processed_count += 1;
        }

        Ok(processed_count)
    }

    #[instrument(skip(self), level = "debug")]
    fn load_texts(&self, path: &str, dry_run: bool, force: bool) -> ApplicationResult<usize> {
        let imports = self
            .import_repository
            .import_text_documents(path)
            .map_err(|e| ApplicationError::Other(format!("Failed to import data: {}", e)))?;

        if dry_run {
            return Ok(imports.len());
        }

        let mut processed_count = 0;

        for import in imports {
            // Check if bookmark with URL already exists
            if let Some(existing) = self.repository.get_by_url(&import.url)? {
                // Calculate content hash for comparison
                let content = get_content_for_embedding(&import);
                let new_hash = calc_content_hash(&content);

                // Only update if force is true or the content has changed
                if force || existing.content_hash.as_ref() != Some(&new_hash) {
                    eprintln!("Processing import: {}", import.url);
                    // Generate embedding
                    let embedding = self
                        .embedder
                        .embed(&content)?
                        .map(|v| serialize_embedding(v).map_err(ApplicationError::from))
                        .transpose()?;

                    // Create updated bookmark
                    let mut updated = existing.clone();
                    updated.title = import.title;
                    updated.description = String::new(); // Don't store content, only embeddings
                    updated.embedding = embedding;
                    updated.embeddable = true;
                    updated.content_hash = Some(new_hash);

                    self.repository.update(&updated)?;
                    processed_count += 1;
                } else {
                    debug!("Skipping import: {} (content unchanged)", import.url);
                }
            } else {
                // Create new bookmark with embedding
                eprintln!("Processing import: {}", import.url);
                let content = get_content_for_embedding(&import);
                let content_hash = Some(calc_content_hash(&content));

                let embedding = self
                    .embedder
                    .embed(&content)?
                    .map(|v| serialize_embedding(v).map_err(ApplicationError::from))
                    .transpose()?;

                let tags = import.tags.clone();
                let mut bookmark = BookmarkBuilder::default()
                    .id(None)
                    .url(import.url)
                    .title(import.title)
                    .description(String::new())
                    .tags(tags)
                    .access_count(0)
                    .created_at(chrono::Utc::now())
                    .updated_at(chrono::Utc::now())
                    .embeddable(true)
                    .embedding(embedding)
                    .content_hash(content_hash)
                    .build()
                    .map_err(|e| ApplicationError::Domain(e.into()))?;

                self.repository.add(&mut bookmark)?;
                processed_count += 1;
            }
        }

        Ok(processed_count)
    }

    #[instrument(skip(self), level = "debug")]
    fn import_files(
        &self,
        paths: &[String],
        update: bool,
        delete_missing: bool,
        dry_run: bool,
        verbose: bool,
        base_path_name: Option<&str>,
    ) -> ApplicationResult<(usize, usize, usize)> {
        use crate::domain::repositories::import_repository::ImportOptions;

        debug!("Starting file import: paths={:?}, update={}, delete_missing={}, dry_run={}, verbose={}, base_path={:?}", 
               paths, update, delete_missing, dry_run, verbose, base_path_name);

        // Load settings for base path resolution
        let settings = crate::config::load_settings(None)
            .map_err(|e| ApplicationError::Other(format!("Failed to load settings: {}", e)))?;

        // Resolve actual scan paths based on base path
        let actual_scan_paths = if let Some(base_name) = base_path_name {
            if let Some(base_value) = settings.base_paths.get(base_name) {
                let expanded_base = crate::config::resolve_file_path(&settings, base_value);
                // Convert relative paths to absolute paths under the base
                paths
                    .iter()
                    .map(|relative_path| {
                        let full_path = std::path::Path::new(&expanded_base).join(relative_path);
                        full_path.to_string_lossy().to_string()
                    })
                    .collect()
            } else {
                return Err(ApplicationError::Other(format!(
                    "Base path '{}' not found in configuration",
                    base_name
                )));
            }
        } else {
            // No base path - use paths as provided
            paths.to_vec()
        };

        let options = ImportOptions {
            update,
            delete_missing,
            dry_run,
            verbose,
        };

        // Get file data from repository using resolved paths
        let file_imports = self
            .import_repository
            .import_files(&actual_scan_paths, &options)
            .map_err(|e| ApplicationError::Other(format!("Failed to scan files: {}", e)))?;

        debug!("Found {} files to process", file_imports.len());

        let mut added_count = 0;
        let mut updated_count = 0;
        let mut deleted_count = 0;

        // Process each file import
        for file_data in &file_imports {
            // Check for duplicate names
            if let Some(existing) = self.find_bookmark_by_name(&file_data.name)? {
                if !update {
                    // Exit with code 65 - duplicate name without --update flag
                    return Err(ApplicationError::DuplicateName {
                        name: file_data.name.clone(),
                        existing_id: existing.id.unwrap_or(-1),
                        file_path: file_data.file_path.display().to_string(),
                    });
                }

                // Check if content or metadata has changed
                let content_changed = existing.file_hash.as_ref() != Some(&file_data.file_hash);
                let metadata_changed = self.has_metadata_changed(&existing, file_data)?;

                if !content_changed && !metadata_changed {
                    debug!("Skipping {}: no changes detected", file_data.name);
                    continue;
                }

                // Report what changed
                if content_changed {
                    println!("Content changed: {}", file_data.name);
                }
                if metadata_changed {
                    println!("Metadata changed: {}", file_data.name);
                }

                // Update existing bookmark
                if !dry_run {
                    self.update_bookmark_from_file(
                        &existing,
                        file_data,
                        &settings,
                        base_path_name,
                    )?;
                }
                updated_count += 1;
                println!("Updated bookmark: {}", file_data.name);
            } else {
                // Create new bookmark
                if !dry_run {
                    self.create_bookmark_from_file(file_data, &settings, base_path_name)?;
                }
                added_count += 1;
                println!("Added bookmark: {}", file_data.name);
            }
        }

        // Handle delete missing functionality
        if delete_missing {
            let orphaned = self.find_orphaned_bookmarks(&actual_scan_paths, &file_imports)?;
            for bookmark in orphaned {
                if !dry_run {
                    if let Some(id) = bookmark.id {
                        self.repository.delete(id)?;
                    }
                }
                deleted_count += 1;
                println!(
                    "Deleted orphaned bookmark: {} ({:?})",
                    bookmark.title, bookmark.id
                );
            }
        }

        Ok((added_count, updated_count, deleted_count))
    }
}

impl<R: BookmarkRepository> BookmarkServiceImpl<R> {
    /// Find bookmark by name (for duplicate detection)
    fn find_bookmark_by_name(&self, name: &str) -> ApplicationResult<Option<Bookmark>> {
        // Search for bookmarks with matching title (name is stored as title)
        let mut query = BookmarkQuery::new();
        // Quote the search term to handle special characters like hyphens
        query.text_query = Some(format!("\"{}\"", name));

        let results = self.repository.search(&query)?;

        // Find exact title match (case-sensitive)
        for bookmark in results {
            if bookmark.title == name {
                return Ok(Some(bookmark));
            }
        }

        Ok(None)
    }

    /// Create a new bookmark from file data
    fn create_bookmark_from_file(
        &self,
        file_data: &FileImportData,
        settings: &crate::config::Settings,
        base_path_name: Option<&str>,
    ) -> ApplicationResult<Bookmark> {
        use crate::domain::system_tag::SystemTag;

        // Convert content type to system tag
        let system_tag = match file_data.content_type.as_str() {
            "_shell_" => SystemTag::Shell,
            "_md_" => SystemTag::Markdown,
            _ => SystemTag::Shell, // Default
        };

        // Prepare tags (including system tag)
        let mut all_tags = file_data.tags.clone();
        all_tags.insert(system_tag.to_tag()?);

        // Store file content in URL column as required
        let mut bookmark = BookmarkBuilder::default()
            .id(None)
            .url(file_data.content.clone())
            .title(file_data.name.clone())
            .description(String::new())
            .tags(all_tags)
            .access_count(0)
            .created_at(Some(chrono::Utc::now()))
            .updated_at(chrono::Utc::now())
            .embedding(None)
            .content_hash(None)
            .embeddable(true)
            .file_path(None)
            .file_mtime(None)
            .file_hash(None)
            .build()
            .map_err(|e| ApplicationError::Other(format!("Failed to build bookmark: {}", e)))?;

        // Set file metadata with base path handling
        let file_path_str = if let Some(base_name) = base_path_name {
            // User provided base path - store as relative path with base path variable
            if let Some(base_value) = settings.base_paths.get(base_name) {
                let expanded_base = crate::config::resolve_file_path(settings, base_value);
                let absolute_file_path = file_data.file_path.display().to_string();

                // Since we resolved the scan paths, files should be under the base path
                if let Some(relative_path) = absolute_file_path.strip_prefix(&expanded_base) {
                    let relative_path = relative_path.strip_prefix('/').unwrap_or(relative_path);
                    crate::config::create_file_path_with_base(base_name, relative_path)
                } else {
                    return Err(ApplicationError::Other(format!(
                        "File {} is not under base path {} ({})",
                        absolute_file_path, base_name, expanded_base
                    )));
                }
            } else {
                return Err(ApplicationError::Other(format!(
                    "Base path '{}' not found in configuration",
                    base_name
                )));
            }
        } else {
            // No base path specified - store absolute path
            file_data.file_path.display().to_string()
        };

        bookmark.file_path = Some(file_path_str);
        bookmark.file_mtime = Some(file_data.file_mtime as i32);
        bookmark.file_hash = Some(file_data.file_hash.clone());

        // Calculate content hash for the bookmark content
        let content_hash = calc_content_hash(&file_data.content);
        bookmark.content_hash = Some(content_hash);

        // Generate embedding if possible
        if bookmark.embeddable {
            let embedding_content = format!("{} {}", bookmark.title, bookmark.url);
            bookmark.embedding = self
                .embedder
                .embed(&embedding_content)?
                .map(|v| serialize_embedding(v).map_err(ApplicationError::from))
                .transpose()?;
        }

        self.repository.add(&mut bookmark)?;
        Ok(bookmark)
    }

    /// Update existing bookmark from file data
    fn update_bookmark_from_file(
        &self,
        existing: &Bookmark,
        file_data: &FileImportData,
        settings: &crate::config::Settings,
        base_path_name: Option<&str>,
    ) -> ApplicationResult<Bookmark> {
        let mut updated = existing.clone();

        // Update content and metadata (content goes in url column for file imports)
        updated.url = file_data.content.clone();
        updated.title = file_data.name.clone();

        // Update file path with base path handling
        let file_path_str = if let Some(base_name) = base_path_name {
            // User provided base path - store as relative path with base path variable
            if let Some(base_value) = settings.base_paths.get(base_name) {
                let expanded_base = crate::config::resolve_file_path(settings, base_value);
                let absolute_file_path = file_data.file_path.display().to_string();

                // Since we resolved the scan paths, files should be under the base path
                if let Some(relative_path) = absolute_file_path.strip_prefix(&expanded_base) {
                    let relative_path = relative_path.strip_prefix('/').unwrap_or(relative_path);
                    crate::config::create_file_path_with_base(base_name, relative_path)
                } else {
                    return Err(ApplicationError::Other(format!(
                        "File {} is not under base path {} ({})",
                        absolute_file_path, base_name, expanded_base
                    )));
                }
            } else {
                return Err(ApplicationError::Other(format!(
                    "Base path '{}' not found in configuration",
                    base_name
                )));
            }
        } else {
            // No base path specified - store absolute path
            file_data.file_path.display().to_string()
        };

        updated.file_path = Some(file_path_str);
        updated.file_mtime = Some(file_data.file_mtime as i32);
        updated.file_hash = Some(file_data.file_hash.clone());

        // Update content hash
        let content_hash = calc_content_hash(&file_data.content);
        updated.content_hash = Some(content_hash);

        // Update tags (merge with existing, keeping system tags)
        let mut new_tags = file_data.tags.clone();
        // Preserve system tags from existing bookmark
        for tag in &existing.tags {
            if tag.value().starts_with('_') && tag.value().ends_with('_') {
                new_tags.insert(tag.clone());
            }
        }
        updated.tags = new_tags;

        // Regenerate embedding if embeddable
        if updated.embeddable {
            let embedding_content = format!("{} {}", updated.title, updated.url);
            updated.embedding = self
                .embedder
                .embed(&embedding_content)?
                .map(|v| serialize_embedding(v).map_err(ApplicationError::from))
                .transpose()?;
        }

        self.repository.update(&updated)?;
        Ok(updated)
    }

    /// Find orphaned bookmarks (file_path set but file no longer exists or not found in current scan)
    fn find_orphaned_bookmarks(
        &self,
        import_paths: &[String],
        current_imports: &[FileImportData],
    ) -> ApplicationResult<Vec<Bookmark>> {
        let all_bookmarks = self.repository.get_all()?;
        let mut orphaned = Vec::new();

        // Create a set of currently imported file paths for quick lookup
        let current_file_paths: HashSet<_> = current_imports
            .iter()
            .map(|import| {
                import
                    .file_path
                    .canonicalize()
                    .unwrap_or_else(|_| import.file_path.clone())
            })
            .collect();

        for bookmark in all_bookmarks {
            if let Some(file_path_str) = &bookmark.file_path {
                // Handle base path variables and resolve to absolute path
                let settings = crate::config::load_settings(None).map_err(|e| {
                    ApplicationError::Other(format!("Failed to load settings: {}", e))
                })?;
                let resolved_path = crate::config::resolve_file_path(&settings, file_path_str);
                let path = Path::new(&resolved_path);

                // Check if file still exists at the stored path
                let file_exists = path.exists();

                // Check if this file was found in the current import scan
                let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                let found_in_scan = current_file_paths.contains(&canonical_path);

                // If the file doesn't exist OR it wasn't found in the current scan, it's orphaned
                if !file_exists || !found_in_scan {
                    // Verify the file was under one of the import paths
                    let should_delete = import_paths.iter().any(|import_path| {
                        path.starts_with(import_path)
                            || path
                                .canonicalize()
                                .unwrap_or_else(|_| path.to_path_buf())
                                .starts_with(
                                    Path::new(import_path)
                                        .canonicalize()
                                        .unwrap_or_else(|_| Path::new(import_path).to_path_buf()),
                                )
                    });

                    if should_delete {
                        orphaned.push(bookmark);
                    }
                }
            }
        }

        Ok(orphaned)
    }

    /// Check if metadata (tags, name, type) has changed
    fn has_metadata_changed(
        &self,
        existing: &Bookmark,
        file_data: &FileImportData,
    ) -> ApplicationResult<bool> {
        // Check if title changed
        if existing.title != file_data.name {
            return Ok(true);
        }

        // Check if tags changed (ignore system tags for comparison)
        let existing_user_tags: HashSet<_> = existing
            .tags
            .iter()
            .filter(|tag| !tag.value().starts_with('_') || !tag.value().ends_with('_'))
            .cloned()
            .collect();
        let file_user_tags: HashSet<_> = file_data
            .tags
            .iter()
            .filter(|tag| !tag.value().starts_with('_') || !tag.value().ends_with('_'))
            .cloned()
            .collect();

        if existing_user_tags != file_user_tags {
            return Ok(true);
        }

        // Check if content type changed (check system tags)
        let existing_has_shell = existing.tags.iter().any(|tag| tag.value() == "_shell_");
        let existing_has_md = existing.tags.iter().any(|tag| tag.value() == "_md_");
        let file_is_shell = file_data.content_type == "_shell_";
        let file_is_md = file_data.content_type == "_md_";

        if (existing_has_shell != file_is_shell) || (existing_has_md != file_is_md) {
            return Ok(true);
        }

        Ok(false)
    }
}
// Helper method
fn get_content_for_embedding(import: &BookmarkImportData) -> String {
    let visible_tags: HashSet<_> = import
        .tags
        .iter()
        .filter(|tag| !tag.value().starts_with('_') && !tag.value().ends_with('_'))
        .cloned()
        .collect();

    let tags_str = Tag::format_tags(&visible_tags);
    format!(
        "{}{} -- {}{}",
        tags_str, import.title, import.content, tags_str
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::embeddings::dummy_provider::DummyEmbedding;
    use crate::infrastructure::repositories::json_import_repository::JsonImportRepository;
    use crate::util::testing::{init_test_env, setup_test_db, EnvGuard};
    use std::collections::HashSet;

    // Helper function to create a BookmarkServiceImpl with a test repository
    fn create_test_service() -> impl BookmarkService {
        let repository = setup_test_db();
        let arc_repository = Arc::new(repository);
        let embedder = Arc::new(DummyEmbedding);
        BookmarkServiceImpl::new(
            arc_repository,
            embedder,
            Arc::new(JsonImportRepository::new()),
        )
    }

    #[test]
    fn given_valid_id_when_get_bookmark_then_returns_correct_bookmark() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let bookmark = service.get_bookmark(1).unwrap();

        // Assert
        assert!(bookmark.is_some(), "Should find bookmark with ID 1");
        let bookmark = bookmark.unwrap();
        assert_eq!(bookmark.id, Some(1));
        assert_eq!(bookmark.url, "https://www.google.com");
        assert_eq!(bookmark.title, "Google");
    }

    #[test]
    fn given_invalid_id_when_get_bookmark_then_returns_none() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let result = service.get_bookmark(999).unwrap();

        // Assert
        assert!(result.is_none(), "Should not find non-existent bookmark");
    }

    #[test]
    fn given_negative_id_when_get_bookmark_then_returns_error() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let result = service.get_bookmark(-1);

        // Assert
        assert!(result.is_err(), "Negative ID should return error");
        match result {
            Err(ApplicationError::Validation(msg)) => {
                assert!(
                    msg.contains("Invalid bookmark ID"),
                    "Error should mention invalid ID"
                );
            }
            _ => panic!("Expected a Validation error"),
        }
    }

    #[test]
    fn given_valid_url_when_get_bookmark_by_url_then_returns_correct_bookmark() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let result = service
            .get_bookmark_by_url("https://www.google.com")
            .unwrap();

        // Assert
        assert!(result.is_some(), "Should find bookmark with URL");
        let bookmark = result.unwrap();
        assert_eq!(bookmark.url, "https://www.google.com");
        assert_eq!(bookmark.title, "Google");
    }

    #[test]
    fn given_new_bookmark_when_add_bookmark_then_creates_and_returns_bookmark() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let url = "https://newbookmark.example.com";
        let title = "New Bookmark";
        let description = "Test description";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        // Act
        let bookmark = service
            .add_bookmark(url, Some(title), Some(description), Some(&tags), false)
            .unwrap();

        // Assert
        assert!(
            bookmark.id.is_some(),
            "Bookmark should have ID after adding"
        );
        assert_eq!(bookmark.url, url);
        assert_eq!(bookmark.title, title);
        assert_eq!(bookmark.description, description);
        assert_eq!(bookmark.tags.len(), 1);
        assert!(bookmark.tags.contains(&Tag::new("test").unwrap()));

        // Verify it can be retrieved
        let retrieved = service.get_bookmark(bookmark.id.unwrap()).unwrap().unwrap();
        assert_eq!(retrieved.url, url);
    }

    #[test]
    fn given_existing_url_when_add_bookmark_then_returns_error() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let existing_url = "https://www.google.com";

        // Act
        let result = service.add_bookmark(
            existing_url,
            Some("Title"),
            Some("Description"),
            None,
            false,
        );

        // Assert
        assert!(result.is_err(), "Adding duplicate URL should fail");
        match result {
            Err(ApplicationError::BookmarkExists(_, url)) => {
                assert_eq!(
                    url, existing_url,
                    "Error message should contain the existing URL"
                );
            }
            _ => panic!("Expected a BookmarkExists error"),
        }
    }

    // #[test]
    // #[serial]
    // fn given_existing_bookmark_when_update_content_then_updates_correctly() {
    //     // Arrange
    //     let _env = init_test_env();
    //     let _guard = EnvGuard::new();
    //     let service = create_test_service();
    //     let id = 1; // Using an existing ID from the test database
    //     let new_title = "Updated Google";
    //     let new_description = "Updated description";
    //
    //     // Act
    //     let updated = service
    //         .update_bookmark_content(id, new_title, new_description)
    //         .unwrap();
    //
    //     // Assert
    //     assert_eq!(updated.title, new_title);
    //     assert_eq!(updated.description, new_description);
    //
    //     // Verify changes were persisted
    //     let retrieved = service.get_bookmark(id).unwrap().unwrap();
    //     assert_eq!(retrieved.title, new_title);
    //     assert_eq!(retrieved.description, new_description);
    // }
    //
    // #[test]
    // #[serial]
    // fn given_non_existent_bookmark_when_update_content_then_returns_error() {
    //     // Arrange
    //     let _env = init_test_env();
    //     let _guard = EnvGuard::new();
    //     let service = create_test_service();
    //
    //     // Act
    //     let result = service.update_bookmark_content(999, "Title", "Description");
    //
    //     // Assert
    //     assert!(
    //         result.is_err(),
    //         "Updating non-existent bookmark should fail"
    //     );
    //     match result {
    //         Err(ApplicationError::BookmarkNotFound(id)) => {
    //             assert_eq!(id, 999);
    //         }
    //         _ => panic!("Expected a BookmarkNotFound error"),
    //     }
    // }

    #[test]
    fn given_existing_bookmark_when_add_tags_then_adds_tags_correctly() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let id = 1; // Using an existing ID from the test database
        let mut tags = HashSet::new();
        tags.insert(Tag::new("newtag").unwrap());

        // Get original tags
        let original = service.get_bookmark(id).unwrap().unwrap();
        let original_tag_count = original.tags.len();

        // Act
        let updated = service.add_tags_to_bookmark(id, &tags).unwrap();

        // Assert
        assert!(updated.tags.contains(&Tag::new("newtag").unwrap()));
        assert_eq!(updated.tags.len(), original_tag_count + 1);

        // Verify changes were persisted
        let retrieved = service.get_bookmark(id).unwrap().unwrap();
        assert!(retrieved.tags.contains(&Tag::new("newtag").unwrap()));
    }

    #[test]
    fn given_existing_bookmark_when_remove_tags_then_removes_tags_correctly() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Find a bookmark with known tags
        let bookmark = service.get_bookmark(1).unwrap().unwrap();
        let tag_to_remove = bookmark.tags.iter().next().unwrap().clone();
        let original_tag_count = bookmark.tags.len();

        // Skip test if no tags to remove
        if original_tag_count == 0 {
            return;
        }

        let mut tags_to_remove = HashSet::new();
        tags_to_remove.insert(tag_to_remove.clone());

        // Act
        let updated = service
            .remove_tags_from_bookmark(1, &tags_to_remove)
            .unwrap();

        // Assert
        assert!(!updated.tags.contains(&tag_to_remove));
        assert_eq!(updated.tags.len(), original_tag_count - 1);

        // Verify changes were persisted
        let retrieved = service.get_bookmark(1).unwrap().unwrap();
        assert!(!retrieved.tags.contains(&tag_to_remove));
    }

    #[test]
    fn given_existing_bookmark_when_replace_tags_then_replaces_all_tags() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let id = 1;

        let mut new_tags = HashSet::new();
        new_tags.insert(Tag::new("replaced1").unwrap());
        new_tags.insert(Tag::new("replaced2").unwrap());

        // Act
        let updated = service.replace_bookmark_tags(id, &new_tags).unwrap();

        // Assert
        assert_eq!(updated.tags.len(), 2);
        assert!(updated.tags.contains(&Tag::new("replaced1").unwrap()));
        assert!(updated.tags.contains(&Tag::new("replaced2").unwrap()));

        // Verify changes were persisted
        let retrieved = service.get_bookmark(id).unwrap().unwrap();
        assert_eq!(retrieved.tags.len(), 2);
        assert!(retrieved.tags.contains(&Tag::new("replaced1").unwrap()));
        assert!(retrieved.tags.contains(&Tag::new("replaced2").unwrap()));
    }

    #[test]
    fn given_existing_bookmark_when_record_access_then_increments_access_count() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let id = 1;

        // Get original access count
        let original = service.get_bookmark(id).unwrap().unwrap();
        let original_count = original.access_count;

        // Act
        let updated = service.record_bookmark_access(id).unwrap();

        // Assert
        assert_eq!(updated.access_count, original_count + 1);

        // Verify changes were persisted
        let retrieved = service.get_bookmark(id).unwrap().unwrap();
        assert_eq!(retrieved.access_count, original_count + 1);
    }

    #[test]
    fn given_test_database_when_delete_bookmark_then_removes_bookmark() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // First add a test bookmark that we can delete
        let url = "https://todelete.example.com";
        let bookmark = service
            .add_bookmark(url, Some("To Delete"), Some("Description"), None, false)
            .unwrap();
        let id = bookmark.id.unwrap();

        // Verify it exists
        assert!(service.get_bookmark(id).unwrap().is_some());

        // Act
        let result = service.delete_bookmark(id).unwrap();

        // Assert
        assert!(result, "Delete should return true on success");

        // Verify it was deleted
        assert!(service.get_bookmark(id).unwrap().is_none());
    }

    // #[test]
    // #[serial]
    // fn given_tags_when_search_by_all_tags_then_returns_matching_bookmarks() {
    //     // Arrange
    //     let _env = init_test_env();
    //     let _guard = EnvGuard::new();
    //     let service = create_test_service();
    //
    //     // Create tags that exist in test data
    //     let mut tags = HashSet::new();
    //     tags.insert(Tag::new("aaa").unwrap());
    //     tags.insert(Tag::new("bbb").unwrap());
    //
    //     // Act
    //     let results = service.search_bookmarks_by_all_tags(&tags).unwrap();
    //
    //     // Assert
    //     assert!(
    //         !results.is_empty(),
    //         "Should find bookmarks with all specified tags"
    //     );
    //
    //     // Every result should have ALL the specified tags
    //     for bookmark in &results {
    //         assert!(bookmark.tags.contains(&Tag::new("aaa").unwrap()));
    //         assert!(bookmark.tags.contains(&Tag::new("bbb").unwrap()));
    //     }
    // }
    //
    // #[test]
    // #[serial]
    // fn given_tags_when_search_by_any_tag_then_returns_matching_bookmarks() {
    //     // Arrange
    //     let _env = init_test_env();
    //     let _guard = EnvGuard::new();
    //     let service = create_test_service();
    //
    //     // Create tags that exist in test data
    //     let mut tags = HashSet::new();
    //     tags.insert(Tag::new("aaa").unwrap());
    //     tags.insert(Tag::new("xxx").unwrap()); // different tag
    //
    //     // Act
    //     let results = service.search_bookmarks_by_any_tag(&tags).unwrap();
    //
    //     // Assert
    //     assert!(
    //         !results.is_empty(),
    //         "Should find bookmarks with any of the specified tags"
    //     );
    //
    //     // Every result should have AT LEAST ONE of the specified tags
    //     for bookmark in &results {
    //         assert!(
    //             bookmark.tags.contains(&Tag::new("aaa").unwrap())
    //                 || bookmark.tags.contains(&Tag::new("xxx").unwrap())
    //         );
    //     }
    //
    //     // Results should include more bookmarks than when searching for all tags
    //     let all_tag_results = service.search_bookmarks_by_all_tags(&tags).unwrap();
    //     assert!(results.len() >= all_tag_results.len());
    // }

    #[test]
    fn given_text_query_when_search_by_text_then_returns_matching_bookmarks() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let results = service.search_bookmarks_by_text("Google").unwrap();

        // Assert
        assert!(
            !results.is_empty(),
            "Should find bookmarks containing the text"
        );

        // At least one result should contain the search text
        let has_match = results.iter().any(|b| {
            b.title.contains("Google")
                || b.description.contains("Google")
                || b.url.contains("Google")
        });
        assert!(
            has_match,
            "At least one result should match the search text"
        );
    }

    #[test]
    fn given_test_database_when_get_all_bookmarks_then_returns_all_bookmarks() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let bookmarks = service.get_all_bookmarks(None, None).unwrap();

        // Assert
        assert!(
            !bookmarks.is_empty(),
            "Should return all bookmarks from test database"
        );

        // Check that we get the expected number based on up.sql
        // The test database from up.sql has 11 sample bookmarks
        assert!(
            bookmarks.len() >= 11,
            "Should return at least the bookmarks from up.sql"
        );
    }

    #[test]
    fn given_count_when_get_random_bookmarks_then_returns_random_selection() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let count = 3;

        // Act
        let bookmarks = service.get_random_bookmarks(count).unwrap();

        // Assert
        assert_eq!(
            bookmarks.len(),
            count,
            "Should return exactly the requested number of bookmarks"
        );

        // Get another random selection and verify it's likely different
        // (This is probabilistic, so there's a small chance it could be the same)
        let another_set = service.get_random_bookmarks(count).unwrap();

        // Convert to sets of IDs for comparison
        let first_ids: HashSet<_> = bookmarks.iter().filter_map(|b| b.id).collect();
        let second_ids: HashSet<_> = another_set.iter().filter_map(|b| b.id).collect();

        // With a decent number of bookmarks, it's very unlikely to get the same random selection twice
        // Only assert if we have enough bookmarks in the test database
        let all_bookmarks = service.get_all_bookmarks(None, None).unwrap();
        if all_bookmarks.len() > count * 3 {
            assert_ne!(
                first_ids, second_ids,
                "Random selections should typically be different"
            );
        }
    }

    #[test]
    fn given_test_database_when_get_bookmarks_without_embeddings_then_returns_correct_bookmarks() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let results = service.get_bookmarks_without_embeddings().unwrap();

        // Assert
        // Verify that all returned bookmarks actually don't have embeddings
        for bookmark in &results {
            assert!(
                bookmark.embedding.is_none(),
                "Returned bookmarks should not have embeddings"
            );
        }
    }

    #[test]
    fn given_bookmark_when_set_embeddable_then_updates_flag() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Create a test bookmark
        let url = "https://embeddingtest.example.com";
        let bookmark = service
            .add_bookmark(
                url,
                Some("Test Embeddable"),
                Some("Description"),
                None,
                false,
            )
            .unwrap();
        let id = bookmark.id.unwrap();

        // Verify initial state
        assert!(!bookmark.embeddable, "Default should be false");

        // Act - Enable embedding
        let updated = service.set_bookmark_embeddable(id, true).unwrap();

        // Assert
        assert!(updated.embeddable, "Flag should be updated to true");

        // Verify persistence
        let retrieved = service.get_bookmark(id).unwrap().unwrap();
        assert!(retrieved.embeddable, "Flag should be persisted as true");

        // Act - Disable embedding
        let updated_again = service.set_bookmark_embeddable(id, false).unwrap();

        // Assert
        assert!(!updated_again.embeddable, "Flag should be updated to false");

        // Verify persistence
        let retrieved_again = service.get_bookmark(id).unwrap().unwrap();
        assert!(
            !retrieved_again.embeddable,
            "Flag should be persisted as false"
        );
    }
}
