use crate::adapter::dal::Dal;
use crate::model::bookmark::BookmarkUpdater;
use crate::adapter::embeddings::{cosine_similarity, deserialize_embedding};
use crate::adapter::json::{bms_to_json, read_ndjson_file_and_create_bookmarks};
use crate::application::dto::{
    BookmarkCreateRequest, BookmarkSearchRequest, BookmarkUpdateRequest
};
use crate::application::services::bookmark_application_service::BookmarkApplicationService;
use crate::cli::args::{Cli, Commands};
use crate::cli::error::{CliError, CliResult};
use crate::context::Context;
use crate::environment::CONFIG;
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
use crate::model::bms::Bookmarks;
use crate::model::tag::Tags;
use crate::service::fzf::fzf_process;
use crate::service::process::{
    delete_bms, edit_bms, open_bm, show_bms, DisplayBookmark, DisplayField, ALL_FIELDS, DEFAULT_FIELDS
};
use crate::util::helper::{confirm, ensure_int_vector};
use camino::Utf8Path;
// src/cli/commands/bookmark_commands.rs
use std::fs::create_dir_all;
use std::io::Write;

use crossterm::style::Stylize;
use itertools::Itertools;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tracing::instrument;

// Helper function to get and validate IDs
fn get_ids(ids: String) -> CliResult<Vec<i32>> {
    ensure_int_vector(&ids.split(',').map(String::from).collect())
        .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
}

// Create a service factory function to reduce boilerplate
fn create_bookmark_service() -> CliResult<BookmarkApplicationService<SqliteBookmarkRepository>> {
    SqliteBookmarkRepository::from_url(&CONFIG.db_url)
        .map_err(|e| CliError::RepositoryError(format!("Failed to create repository: {}", e)))
        .map(BookmarkApplicationService::new)
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
    } = cli.command.unwrap() {
        let mut fields = DEFAULT_FIELDS.to_vec();

        // Combine prefix tags with tags_all if present
        let tags_all = tags_prefix.map_or(tags_all.clone().unwrap_or_default(), |prefix| {
            tags_all.map_or(prefix.clone(), |all| format!("{},{}", all, prefix))
        });

        // Create search request for application service
        let search_request = BookmarkSearchRequest {
            query: fts_query,
            all_tags: if tags_all.is_empty() { None } else { Some(vec![tags_all]) },
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

        // Convert to traditional model for display compatibility
        let mut bms = Bookmarks::new(String::new());
        bms.bms = response.bookmarks.iter().map(|item| {
            let mut bm = crate::model::bookmark::Bookmark::default();
            bm.id = item.id.unwrap_or(0);
            bm.URL = item.url.clone();
            bm.metadata = item.title.clone();
            bm.tags = item.tags.join(",");
            bm
        }).collect();

        if order_desc || order_asc {
            fields.push(DisplayField::LastUpdateTs);
        }

        // Handle different output modes
        match (is_fuzzy, is_json) {
            (true, _) => {
                fzf_process(&bms.bms);
                return Ok(());
            }
            (_, true) => {
                bms_to_json(&bms.bms);
                return Ok(());
            }
            _ => {
                let d_bms: Vec<DisplayBookmark> = bms.bms.iter().map(DisplayBookmark::from).collect();
                show_bms(&d_bms, &fields);
                eprintln!("Found {} bookmarks", bms.bms.len());

                if non_interactive {
                    let ids = bms
                        .bms
                        .iter()
                        .map(|bm| bm.id.to_string())
                        .sorted()
                        .join(",");
                    println!("{}", ids);
                } else {
                    stderr.set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                        .map_err(|e| CliError::Other(format!("Failed to set color: {}", e)))?;
                    writeln!(&mut stderr, "Selection: ")
                        .map_err(|e| CliError::Other(format!("Failed to write to stderr: {}", e)))?;
                    stderr.reset()
                        .map_err(|e| CliError::Other(format!("Failed to reset color: {}", e)))?;
                    crate::service::process::process(&bms.bms);
                }
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn semantic_search(mut stderr: StandardStream, cli: Cli) -> CliResult<()> {
    if let Commands::SemSearch { query, limit, non_interactive } = cli.command.unwrap() {
        let bms = Bookmarks::new(String::new());
        let results = find_similar(&query, &bms)?;
        let limit = limit.unwrap_or(10) as usize;

        let filtered_results: Vec<_> = results
            .iter()
            .filter_map(|(id, similarity)| {
                bms.bms.iter().find(|bm| bm.id == *id).map(|bm| {
                    let mut dbm = DisplayBookmark::from(bm);
                    dbm.similarity = Some(*similarity);
                    (bm.clone(), dbm)
                })
            })
            .take(limit)
            .collect();

        // Display results
        let display_bookmarks: Vec<_> = filtered_results
            .iter()
            .map(|(_, dbm)| dbm)
            .cloned()
            .collect();
        show_bms(&display_bookmarks, &DEFAULT_FIELDS);

        if non_interactive {
            let ids = filtered_results
                .iter()
                .map(|(bm, _)| bm.id.to_string())
                .sorted()
                .join(",");
            println!("{}", ids);
        } else {
            stderr.set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                .map_err(|e| CliError::Other(format!("Failed to set color: {}", e)))?;
            writeln!(&mut stderr, "Selection: ")
                .map_err(|e| CliError::Other(format!("Failed to write to stderr: {}", e)))?;
            stderr.reset()
                .map_err(|e| CliError::Other(format!("Failed to reset color: {}", e)))?;
            crate::service::process::process(
                &filtered_results
                    .into_iter()
                    .map(|(bm, _)| bm)
                    .collect::<Vec<_>>(),
            );
        }
    }
    Ok(())
}

#[instrument]
fn find_similar(query: &str, bms: &Bookmarks) -> CliResult<Vec<(i32, f32)>> {
    // Ensure we have a context with OpenAI embedding
    let embedding = Context::read_global()
        .execute(query)
        .map_err(|e| CliError::Other(e.to_string()))?
        .ok_or_else(|| CliError::CommandFailed("No embedding generated. OpenAI flag set?".to_string()))?;

    let query_vector = ndarray::Array1::from(embedding);
    let mut results = Vec::with_capacity(bms.bms.len());

    for bm in &bms.bms {
        if let Some(embedding_data) = &bm.embedding {
            let bm_embedding = deserialize_embedding(embedding_data.clone())
                .map_err(|e| CliError::CommandFailed(format!("Failed to deserialize embedding: {}", e)))?;
            let bm_vector = ndarray::Array1::from(bm_embedding);
            let similarity = cosine_similarity(&query_vector, &bm_vector);
            results.push((bm.id, similarity));
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results)
}

#[instrument(skip(cli))]
pub fn open(cli: Cli) -> CliResult<()> {
    if let Commands::Open { ids } = cli.command.unwrap() {
        let mut dal = Dal::new(CONFIG.db_url.clone());
        for id in get_ids(ids)? {
            open_bm(&dal.get_bookmark_by_id(id)
                .map_err(|e| CliError::CommandFailed(format!("Failed to get bookmark: {}", e)))?)
                .map_err(|e| CliError::CommandFailed(format!("Failed to open bookmark: {}", e)))?;
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn add(cli: Cli) -> CliResult<()> {
    if let Commands::Add { url, tags, title, desc, no_web, edit } = cli.command.unwrap() {
        // Create service
        let service = create_bookmark_service()?;
        let mut dal = Dal::new(CONFIG.db_url.clone());

        // Check for unknown tags
        let normalized_tags = Tags::normalize_tag_string(tags.clone());
        let unknown_tags = Bookmarks::new(String::new())
            .check_tags(normalized_tags.clone())
            .map_err(|e| CliError::CommandFailed(format!("Failed to check tags: {}", e)))?;

        if !unknown_tags.is_empty() && !confirm(&format!("Unknown tags: {:?}, create?", unknown_tags)) {
            return Err(CliError::OperationAborted);
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
            tags: if normalized_tags.is_empty() { None } else { Some(normalized_tags) },
            fetch_metadata: Some(!no_web),
        };

        // Execute add
        let result = service.add_bookmark(request)?;
        println!("Added bookmark: {}", result.id.unwrap_or(0));

        // Handle editing option
        if edit && result.id.is_some() {
            edit_bms(vec![result.id.unwrap()], vec![])
                .map_err(|e| CliError::CommandFailed(format!("Failed to edit bookmark: {}", e)))?;
        }

        // Show the bookmark for confirmation
        if let Some(id) = result.id {
            let bm = dal.get_bookmark_by_id(id)
                .map_err(|e| CliError::CommandFailed(format!("Failed to get bookmark: {}", e)))?;
            show_bms(
                &vec![DisplayBookmark::from(&bm)],
                &DEFAULT_FIELDS,
            );
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn delete(cli: Cli) -> CliResult<()> {
    if let Commands::Delete { ids } = cli.command.unwrap() {
        let ids = get_ids(ids)?;
        delete_bms(ids, Bookmarks::new(String::new()).bms)
            .map_err(|e| CliError::CommandFailed(format!("Failed to delete bookmarks: {}", e)))?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn update(cli: Cli) -> CliResult<()> {
    if let Commands::Update { ids, tags, tags_not, force } = cli.command.unwrap() {
        // Validate force update requirements
        if force && (tags.is_none() || tags_not.is_some()) {
            return Err(CliError::InvalidInput("Force update requires tags but no ntags".to_string()));
        }

        let ids = get_ids(ids)?;
        let tags = Tags::normalize_tag_string(tags);
        let tags_not = Tags::normalize_tag_string(tags_not);

        // Application service approach
        let service = create_bookmark_service()?;

        for id in ids.clone() {
            // Get the bookmark
            let bookmark = service.get_bookmark(id)?
                .ok_or_else(|| CliError::CommandFailed(format!("Bookmark with ID {} not found", id)))?;

            // Get current tags
            let mut current_tags: Vec<String> = bookmark.tags;

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
        edit_bms(get_ids(ids)?, Bookmarks::new(String::new()).bms)
            .map_err(|e| CliError::CommandFailed(format!("Failed to edit bookmarks: {}", e)))?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn show(cli: Cli) -> CliResult<()> {
    if let Commands::Show { ids } = cli.command.unwrap() {
        let mut dal = Dal::new(CONFIG.db_url.clone());
        let mut bms = Vec::new();

        for id in get_ids(ids)? {
            if let Ok(bm) = dal.get_bookmark_by_id(id) {
                bms.push(bm);
            } else {
                eprintln!("Bookmark with id {} not found", id);
            }
        }

        show_bms(
            &bms.iter().map(DisplayBookmark::from).collect::<Vec<_>>(),
            &ALL_FIELDS,
        );
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn surprise(cli: Cli) -> CliResult<()> {
    if let Commands::Surprise { n } = cli.command.unwrap() {
        let mut dal = Dal::new(CONFIG.db_url.clone());
        let bms = dal.get_randomized_bookmarks(n)
            .map_err(|e| CliError::CommandFailed(format!("Failed to get randomized bookmarks: {}", e)))?;

        for bm in &bms {
            open::that(&bm.URL)
                .map_err(|e| CliError::CommandFailed(format!("Failed to open URL {}: {}", bm.URL, e)))?;
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn create_db(cli: Cli) -> CliResult<()> {
    if let Commands::CreateDb { path } = cli.command.unwrap() {
        let path = Utf8Path::new(&path);
        if path.exists() {
            return Err(CliError::InvalidInput(format!("Database already exists at {:?}", path)));
        }

        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .map_err(|e| CliError::CommandFailed(format!("Failed to create parent directories: {}", e)))?;
        }

        let mut dal = Dal::new(path.to_string());
        crate::adapter::dal::migration::init_db(&mut dal.conn)
            .map_err(|e| CliError::CommandFailed(format!("Failed to initialize database: {}", e)))?;
        dal.clean_table()
            .map_err(|e| CliError::CommandFailed(format!("Failed to clean table: {}", e)))?;

        println!("Database created at {:?}", path);
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn backfill(cli: Cli) -> CliResult<()> {
    if let Commands::Backfill { dry_run } = cli.command.unwrap() {
        let mut dal = Dal::new(CONFIG.db_url.clone());
        let bms = dal.get_bookmarks_without_embedding()
            .map_err(|e| CliError::CommandFailed(format!("Failed to get bookmarks without embedding: {}", e)))?;

        for bm in &bms {
            println!("Updating: {:?}", bm.metadata);
            if !dry_run {
                let mut bm = bm.clone();
                bm.update();
                dal.update_bookmark(bm)
                    .map_err(|e| CliError::CommandFailed(format!("Failed to update bookmark: {}", e)))?;
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
            crate::service::embeddings::create_embeddings_for_non_bookmarks(path)
                .map_err(|e| CliError::CommandFailed(format!("Failed to create embeddings: {}", e)))
        }
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::embeddings::OpenAiEmbedding;
    use camino::Utf8PathBuf;
    use camino_tempfile::tempdir;
    use fs_extra::{copy_items, dir};
    use rstest::*;
    use std::fs;

    #[fixture]
    fn temp_dir() -> Utf8PathBuf {
        let tempdir = tempdir().unwrap();
        let options = dir::CopyOptions::new().overwrite(true);
        copy_items(
            &[
                "tests/resources/bkmr.v1.db",
                "tests/resources/bkmr.v2.db",
                "tests/resources/bkmr.v2.noembed.db",
            ],
            "../db",
            &options,
        )
        .expect("Failed to copy test project directory");

        tempdir.into_path()
    }

    #[test]
    fn test_get_ids_valid() {
        let result = get_ids("1,2,3".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_get_ids_invalid() {
        let result = get_ids("1,abc,3".to_string());
        assert!(result.is_err());
        match result {
            Err(CliError::InvalidIdFormat(_)) => {}, // Expected error
            _ => panic!("Expected InvalidIdFormat error"),
        }
    }

    // #[allow(unused_variables)]
    // #[ignore = "requires database and OpenAI setup"]
    // #[rstest]
    // fn test_find_similar(temp_dir: Utf8PathBuf) -> CliResult<()> {
    //     // Given: Set up test environment
    //     fs::rename("../db/bkmr.v2.db", "../db/bkmr.db").map_err(|e| CliError::Io(e))?;
    //     let bms = Bookmarks::new("".to_string());
    //
    //     // Initialize context with OpenAI
    //     Context::update_global(Context::new(Box::new(OpenAiEmbedding::default())))
    //         .map_err(|e| CliError::Domain(e))?;
    //
    //     // Execute search
    //     let results = find_similar("test query", &bms)?;
    //
    //     // Basic validation
    //     assert!(!results.is_empty(), "Expected non-empty results");
    //
    //     // Verify that results are ordered by similarity (descending)
    //     let similarities: Vec<_> = results.iter().map(|(_, sim)| *sim).collect();
    //     assert!(
    //         similarities.windows(2).all(|w| w[0] >= w[1]),
    //         "Expected similarities to be in descending order"
    //     );
    //
    //     Ok(())
    // }
}