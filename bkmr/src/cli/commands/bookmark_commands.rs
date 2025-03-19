// src/cli/commands/bookmark_commands.rs

use std::fs::create_dir_all;
use std::io::Write;
use std::path::Path;

use crate::application::dto::{
    BookmarkCreateRequest, BookmarkResponse, BookmarkSearchRequest, BookmarkUpdateRequest,
};
use crate::application::services::bookmark_application_service::BookmarkApplicationService;
use crate::cli::args::{Cli, Commands};
use crate::cli::display::{DisplayBookmark, DisplayField, ALL_FIELDS, DEFAULT_FIELDS};
use crate::cli::error::{CliError, CliResult};
use crate::cli::fzf::fzf_process;
use crate::cli::process::{delete_bookmarks, edit_bookmarks, open_bookmark, process};
use crate::context::Context;
use crate::environment::CONFIG;
use crate::infrastructure::embeddings::{
    cosine_similarity, deserialize_embedding, OpenAiEmbedding,
};
use crate::infrastructure::json::{bms_to_json, read_ndjson_file_and_create_bookmarks};
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
use crate::infrastructure::repositories::sqlite::migration::init_db;
use crate::util::helper::{confirm, ensure_int_vector};

use crate::cli::create_bookmark_service;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use itertools::Itertools;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tracing::instrument;

// Helper function to get and validate IDs
fn get_ids(ids: String) -> CliResult<Vec<i32>> {
    ensure_int_vector(&ids.split(',').map(String::from).collect())
        .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
}

#[instrument(skip(cli))]
pub fn search(mut stderr: StandardStream, cli: Cli) -> CliResult<()> {
    // Extract all arguments from Commands::Search
    if let Commands::Search {
        fts_query,
        tags_exact,
        tags_all,
        tags_all_not,
        tags_any,
        tags_any_not,
        tags_prefix,
        order_desc,
        order_asc,
        non_interactive,
        is_fuzzy,
        is_json,
        limit,
    } = cli.command.unwrap()
    {
        let mut fields = DEFAULT_FIELDS.to_vec();

        // Combine prefix tags with tags_all if present
        let tags_all = tags_prefix.map_or(tags_all.clone().unwrap_or_default(), |prefix| {
            tags_all.map_or(prefix.clone(), |all| format!("{},{}", all, prefix))
        });

        // Create search request for application service
        let search_request = BookmarkSearchRequest {
            query: fts_query,
            all_tags: if tags_all.is_empty() {
                None
            } else {
                Some(vec![tags_all])
            },
            any_tags: tags_any.map(|t| vec![t]),
            exclude_all_tags: tags_all_not.map(|t| vec![t]),
            exclude_any_tags: tags_any_not.map(|t| vec![t]),
            exact_tags: tags_exact.map(|t| vec![t]),
            sort_by_date: Some(order_desc || order_asc),
            sort_descending: Some(order_desc),
            limit: limit.map(|l| l as usize),
            offset: None,
        };

        // Get service and execute search
        let service = create_bookmark_service()?;
        let response = service.search_bookmarks(search_request)?;

        // Convert DTOs to DisplayBookmark for presentation
        let bookmarks = response
            .bookmarks
            .iter()
            .map(|item| {
                let mut display_bookmark = DisplayBookmark::default();
                display_bookmark.id = item.id.unwrap_or(0);
                display_bookmark.url = item.url.clone();
                display_bookmark.title = item.title.clone();
                display_bookmark.tags = item.tags.join(",");
                display_bookmark
            })
            .collect::<Vec<DisplayBookmark>>();

        if order_desc || order_asc {
            fields.push(DisplayField::LastUpdateTs);
        }

        // Handle different output modes
        match (is_fuzzy, is_json) {
            (true, _) => {
                // Get full bookmark details for fzf
                let bookmark_responses = response
                    .bookmarks
                    .iter()
                    .filter_map(|item| {
                        if let Some(id) = item.id {
                            service.get_bookmark(id).ok().flatten()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                fzf_process(&bookmark_responses)?;
                return Ok(());
            }
            (_, true) => {
                // Get full bookmark details for JSON output
                let bookmark_responses = response
                    .bookmarks
                    .iter()
                    .filter_map(|item| {
                        if let Some(id) = item.id {
                            service.get_bookmark(id).ok().flatten()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                bms_to_json(&bookmark_responses)?;
                return Ok(());
            }
            _ => {
                crate::cli::display::show_bookmarks(&bookmarks, &fields);
                eprintln!("Found {} bookmarks", bookmarks.len());

                if non_interactive {
                    let ids = response
                        .bookmarks
                        .iter()
                        .filter_map(|bm| bm.id)
                        .map(|id| id.to_string())
                        .sorted()
                        .join(",");
                    println!("{}", ids);
                } else {
                    stderr
                        .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                        .map_err(|e| CliError::Other(format!("Failed to set color: {}", e)))?;
                    writeln!(&mut stderr, "Selection: ").map_err(|e| {
                        CliError::Other(format!("Failed to write to stderr: {}", e))
                    })?;
                    stderr
                        .reset()
                        .map_err(|e| CliError::Other(format!("Failed to reset color: {}", e)))?;

                    // Get full bookmark details for processing
                    let bookmark_responses = response
                        .bookmarks
                        .iter()
                        .filter_map(|item| {
                            if let Some(id) = item.id {
                                service.get_bookmark(id).ok().flatten()
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    process(&bookmark_responses)?;
                }
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn semantic_search(mut stderr: StandardStream, cli: Cli) -> CliResult<()> {
    if let Commands::SemSearch {
        query,
        limit,
        non_interactive,
    } = cli.command.unwrap()
    {
        // Set up OpenAI embedding for semantic search
        Context::update_global(Context::new(Box::new(OpenAiEmbedding::default())))
            .map_err(|e| CliError::Other(format!("Failed to set OpenAI context: {}", e)))?;

        // Create service
        let service = create_bookmark_service()?;

        // Let's create a simple DTO for semantic search results
        #[derive(Debug)]
        struct SemanticSearchResult {
            bookmark_response: BookmarkResponse,
            similarity: f32,
        }

        // Get embedding for query
        let embedding = Context::read_global()
            .execute(&query)
            .map_err(|e| CliError::Other(e.to_string()))?
            .ok_or_else(|| {
                CliError::CommandFailed("No embedding generated. OpenAI flag set?".to_string())
            })?;

        // Use a specific application service method for semantic search if available,
        // or create one if necessary

        // For now, we'll simulate the semantic search with what we have
        let query_vector = ndarray::Array1::from(embedding);
        let mut results = Vec::new();

        // Get all bookmarks through the service
        let all_bookmarks = service.get_all_bookmarks()?;

        // Calculate similarity for each bookmark
        for bookmark in all_bookmarks {
            if let Some(id) = bookmark.id {
                if let Some(embedding_bytes) =
                    get_embedding_for_bookmark_from_service(&service, id)?
                {
                    let bm_embedding = deserialize_embedding(embedding_bytes).map_err(|e| {
                        CliError::CommandFailed(format!("Failed to deserialize embedding: {}", e))
                    })?;
                    let bm_vector = ndarray::Array1::from(bm_embedding);
                    let similarity = cosine_similarity(&query_vector, &bm_vector);
                    results.push(SemanticSearchResult {
                        bookmark_response: bookmark,
                        similarity,
                    });
                }
            }
        }

        // Sort results by similarity
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit
        let limit = limit.unwrap_or(10) as usize;
        let limited_results = results.into_iter().take(limit).collect::<Vec<_>>();

        // Convert to display format
        let mut display_bookmarks = Vec::new();
        let mut bookmark_responses = Vec::new();

        for result in &limited_results {
            let mut display_bookmark = DisplayBookmark::from_dto(&result.bookmark_response);
            display_bookmark.similarity = Some(result.similarity);
            display_bookmarks.push(display_bookmark);
            bookmark_responses.push(result.bookmark_response.clone());
        }

        // Display results
        crate::cli::display::show_bookmarks(&display_bookmarks, DEFAULT_FIELDS);

        if non_interactive {
            let ids = limited_results
                .iter()
                .filter_map(|r| r.bookmark_response.id)
                .map(|id| id.to_string())
                .sorted()
                .join(",");
            println!("{}", ids);
        } else {
            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                .map_err(|e| CliError::Other(format!("Failed to set color: {}", e)))?;
            writeln!(&mut stderr, "Selection: ")
                .map_err(|e| CliError::Other(format!("Failed to write to stderr: {}", e)))?;
            stderr
                .reset()
                .map_err(|e| CliError::Other(format!("Failed to reset color: {}", e)))?;
            process(&bookmark_responses)?;
        }
    }
    Ok(())
}
// Helper function to get embedding via service
fn get_embedding_for_bookmark_from_service(
    service: &BookmarkApplicationService<SqliteBookmarkRepository>,
    id: i32,
) -> CliResult<Option<Vec<u8>>> {
    // Ideally, we would add this method to the application service
    // For now, we'll use the existing repository approach
    let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url)
        .map_err(|e| CliError::RepositoryError(format!("Failed to get repository: {}", e)))?;

    get_embedding_for_bookmark(&repository, id)
}

// Helper function to get embedding bytes for a bookmark using repository
fn get_embedding_for_bookmark(
    repo: &SqliteBookmarkRepository,
    id: i32,
) -> CliResult<Option<Vec<u8>>> {
    use crate::application::services::embedding_service::EmbeddingService;

    // Create the embedding service
    let embedding_service = EmbeddingService::new(repo.clone());

    // Use the existing method
    embedding_service
        .get_bookmark_embedding(id)
        .map_err(CliError::Application)
}

#[instrument(skip(cli))]
pub fn open(cli: Cli) -> CliResult<()> {
    if let Commands::Open { ids } = cli.command.unwrap() {
        let service = create_bookmark_service()?;
        for id in get_ids(ids)? {
            if let Some(bookmark) = service.get_bookmark(id)? {
                open_bookmark(&bookmark)?;
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn add(cli: Cli) -> CliResult<()> {
    if let Commands::Add {
        url,
        tags,
        title,
        desc,
        no_web,
        edit,
    } = cli.command.unwrap()
    {
        // Create service
        let service = create_bookmark_service()?;

        // Check if bookmark already exists
        let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url).map_err(|e| {
            CliError::RepositoryError(format!("Failed to create repository: {}", e))
        })?;

        if repository.exists_by_url(&url)? {
            return Err(CliError::CommandFailed(format!(
                "Bookmark already exists: {}",
                url
            )));
        }

        // Check for unknown tags
        if let Some(ref tags_str) = tags {
            let repo_tags = repository.get_all_tags()?;
            let known_tags: Vec<String> = repo_tags
                .into_iter()
                .map(|(tag, _)| tag.value().to_string())
                .collect();

            let new_tags: Vec<String> = tags_str
                .split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty() && !known_tags.contains(t))
                .collect();

            if !new_tags.is_empty() && !confirm(&format!("Unknown tags: {:?}, create?", new_tags)) {
                return Err(CliError::OperationAborted);
            }
        }

        // Get web details if needed
        let (web_title, web_desc, _) = if !no_web {
            crate::load_url_details(&url).unwrap_or_default()
        } else {
            Default::default()
        };

        // Create request
        let request = BookmarkCreateRequest {
            url: url.clone(),
            title: title.or(Some(web_title)),
            description: desc.or(Some(web_desc)),
            tags: tags.map(|t| {
                t.split(',')
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect::<Vec<_>>()
            }),
            fetch_metadata: Some(!no_web),
        };

        // Execute add
        let result = service.add_bookmark(request)?;
        println!("Added bookmark: {}", result.id.unwrap_or(0));

        // Handle editing option
        if edit && result.id.is_some() {
            edit_bookmarks(vec![result.id.unwrap()])
                .map_err(|e| CliError::CommandFailed(format!("Failed to edit bookmark: {}", e)))?;
        }

        // Show the bookmark for confirmation
        if let Some(id) = result.id {
            if let Some(bookmark) = service.get_bookmark(id)? {
                crate::cli::display::show_bookmarks(
                    &[DisplayBookmark::from_dto(&bookmark)],
                    DEFAULT_FIELDS,
                );
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn delete(cli: Cli) -> CliResult<()> {
    if let Commands::Delete { ids } = cli.command.unwrap() {
        let service = create_bookmark_service()?;
        let ids = get_ids(ids)?;

        let mut bookmarks = Vec::new();
        for id in &ids {
            if let Some(bookmark) = service.get_bookmark(*id)? {
                bookmarks.push(bookmark);
            }
        }

        // Convert DTOs to domain objects for the delete function
        let domain_bookmarks = bookmarks
            .iter()
            .filter_map(|dto| {
                let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url).ok()?;
                repository.get_by_id(dto.id.unwrap_or(0)).ok().flatten()
            })
            .collect::<Vec<_>>();

        delete_bookmarks(ids)
            .map_err(|e| CliError::CommandFailed(format!("Failed to delete bookmarks: {}", e)))?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn update(cli: Cli) -> CliResult<()> {
    if let Commands::Update {
        ids,
        tags,
        tags_not,
        force,
    } = cli.command.unwrap()
    {
        // Validate force update requirements
        if force && (tags.is_none() || tags_not.is_some()) {
            return Err(CliError::InvalidInput(
                "Force update requires tags but no ntags".to_string(),
            ));
        }

        let ids = get_ids(ids)?;

        // Process tags
        let tags = tags
            .map(|t| {
                t.split(',')
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let tags_not = tags_not
            .map(|t| {
                t.split(',')
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Application service approach
        let service = create_bookmark_service()?;

        for id in ids.clone() {
            // Get the bookmark
            let bookmark = service.get_bookmark(id)?.ok_or_else(|| {
                CliError::CommandFailed(format!("Bookmark with ID {} not found", id))
            })?;

            // Get current tags
            let mut current_tags = bookmark.tags.clone();

            // Apply changes
            let updated_tags = if force {
                // Just use the new tags
                tags.clone()
            } else {
                // Add tags
                for tag in &tags {
                    if !current_tags.contains(tag) {
                        current_tags.push(tag.clone());
                    }
                }

                // Remove tags
                current_tags.retain(|tag| !tags_not.contains(tag));

                current_tags
            };

            // Create update request
            let request = BookmarkUpdateRequest {
                id,
                title: None,
                description: None,
                tags: Some(updated_tags),
            };

            // Update the bookmark
            service.update_bookmark(request)?;
        }

        println!("Updated {} bookmark(s)", ids.len());
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn edit(cli: Cli) -> CliResult<()> {
    if let Commands::Edit { ids } = cli.command.unwrap() {
        let service = create_bookmark_service()?;
        let ids = get_ids(ids)?;

        let domain_bookmarks = ids
            .iter()
            .filter_map(|id| {
                let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url).ok()?;
                repository.get_by_id(*id).ok().flatten()
            })
            .collect::<Vec<_>>();

        edit_bookmarks(ids)
            .map_err(|e| CliError::CommandFailed(format!("Failed to edit bookmarks: {}", e)))?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn show(cli: Cli) -> CliResult<()> {
    if let Commands::Show { ids } = cli.command.unwrap() {
        let service = create_bookmark_service()?;
        let mut bookmarks = Vec::new();

        for id in get_ids(ids)? {
            if let Some(bookmark) = service.get_bookmark(id)? {
                let display_bookmark = DisplayBookmark::from_dto(&bookmark);
                bookmarks.push(display_bookmark);
            } else {
                eprintln!("Bookmark with id {} not found", id);
            }
        }

        crate::cli::display::show_bookmarks(&bookmarks, ALL_FIELDS);
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn surprise(cli: Cli) -> CliResult<()> {
    if let Commands::Surprise { n } = cli.command.unwrap() {
        let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url).map_err(|e| {
            CliError::RepositoryError(format!("Failed to create repository: {}", e))
        })?;

        let bookmarks = repository.get_random(n as usize)?;

        for bookmark in &bookmarks {
            open::that(bookmark.url()).map_err(|e| {
                CliError::CommandFailed(format!("Failed to open URL {}: {}", bookmark.url(), e))
            })?;
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn create_db(cli: Cli) -> CliResult<()> {
    if let Commands::CreateDb { path } = cli.command.unwrap() {
        let path = Path::new(&path);
        if path.exists() {
            return Err(CliError::InvalidInput(format!(
                "Database already exists at {:?}",
                path
            )));
        }

        if let Some(parent) = path.parent() {
            create_dir_all(parent).map_err(|e| {
                CliError::CommandFailed(format!("Failed to create parent directories: {}", e))
            })?;
        }

        let database_url = path.to_string_lossy().to_string();
        let repository = SqliteBookmarkRepository::from_url(&database_url).map_err(|e| {
            CliError::RepositoryError(format!("Failed to create repository: {}", e))
        })?;

        // Get connection and initialize database
        let mut conn = repository
            .get_connection()
            .map_err(|e| CliError::RepositoryError(format!("Failed to get connection: {}", e)))?;

        init_db(&mut conn).map_err(|e| {
            CliError::CommandFailed(format!("Failed to initialize database: {}", e))
        })?;

        // Clean table
        repository
            .clean_table()
            .map_err(|e| CliError::CommandFailed(format!("Failed to clean table: {}", e)))?;

        println!("Database created at {:?}", path);
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn backfill(cli: Cli) -> CliResult<()> {
    if let Commands::Backfill { dry_run } = cli.command.unwrap() {
        let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url).map_err(|e| {
            CliError::RepositoryError(format!("Failed to create repository: {}", e))
        })?;

        let bookmarks = repository.get_without_embeddings()?;

        for bookmark in &bookmarks {
            println!("Updating: {:?}", bookmark.title());
            if !dry_run {
                // Create content for embedding
                let content = bookmark.get_content_for_embedding();

                // Generate embedding
                if let Some(embedding) = Context::read_global().get_embedding(&content) {
                    // Need to update the bookmark with the new embedding
                    let id = bookmark.id().unwrap_or(0);

                    // Create an updated bookmark with embedding
                    let service = BookmarkApplicationService::new(repository.clone());
                    let response = service.get_bookmark(id)?;

                    if let Some(bookmark_dto) = response {
                        // Update the bookmark
                        let request = BookmarkUpdateRequest {
                            id,
                            title: None,
                            description: None,
                            tags: None,
                        };

                        service.update_bookmark(request)?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn load_texts(cli: Cli) -> CliResult<()> {
    if let Commands::LoadTexts { dry_run, path } = cli.command.unwrap() {
        if dry_run {
            let bms = read_ndjson_file_and_create_bookmarks(&path)
                .map_err(|e| CliError::CommandFailed(format!("Failed to read JSON file: {}", e)))?;
            eprintln!("Would load {} texts for semantic search.", bms.len());
            Ok(())
        } else {
            // todo!()
            eprintln!("xxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
            Ok(())
            // create_embeddings_for_non_bookmarks(path)
            //     .map_err(|e| CliError::CommandFailed(format!("Failed to create embeddings: {}", e)))
        }
    } else {
        Ok(())
    }
}

// Extension for DisplayBookmark to support conversion from DTO
// impl DisplayBookmark {
// pub fn from_dto(dto: &crate::application::dto::BookmarkResponse) -> Self {
//     Self {
//         id: dto.id.unwrap_or(0),
//         url: dto.url.clone(),
//         title: dto.title.clone(),
//         description: dto.description.clone(),
//         tags: dto.tags.join(","),
//         access_count: dto.access_count,
//         last_update_ts: dto.updated_at,
//         similarity: None,
//     }
// }

// pub fn from_domain(bookmark: &crate::domain::bookmark::Bookmark) -> Self {
//     Self {
//         id: bookmark.id().unwrap_or(0),
//         url: bookmark.url().to_string(),
//         title: bookmark.title().to_string(),
//         description: bookmark.description().to_string(),
//         tags: bookmark.formatted_tags(),
//         access_count: bookmark.access_count(),
//         last_update_ts: bookmark.updated_at(),
//         similarity: None,
//     }
// }
// }
