// src/cli/bookmark_commands.rs
use crate::app_state::AppState;
use crate::application::services::factory;
use crate::application::services::factory::create_bookmark_service;
use crate::application::templates::bookmark_template::BookmarkTemplate;
use crate::cli::args::{Cli, Commands};
use crate::cli::display::{show_bookmarks, DisplayBookmark, DisplayField, DEFAULT_FIELDS};
use crate::cli::error::{CliError, CliResult};
use crate::cli::fzf::fzf_process;
use crate::cli::process::{edit_bookmarks, open_bookmark, process};
use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::query::SortDirection;
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::search::SemanticSearch;
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::infrastructure::json::{write_bookmarks_as_json, JsonBookmarkView};
use crate::infrastructure::repositories::sqlite::migration;
use crate::infrastructure::repositories::sqlite::repository::{
    print_db_schema, SqliteBookmarkRepository,
};
use crate::util::helper::{confirm, ensure_int_vector};
use crossterm::style::Stylize;
use itertools::Itertools;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::{fs, io};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tracing::instrument;

// Helper function to get and validate IDs
fn get_ids(ids: String) -> CliResult<Vec<i32>> {
    // ensure_int_vector(&ids.split(',').map(String::from).collect())
    //     .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
    let string_vec: Vec<String> = ids.split(',').map(|s| s.trim().to_string()).collect();
    ensure_int_vector(&string_vec)
        .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
}

// Parse a tag string into a HashSet of Tag objects
#[instrument(level = "trace")]
pub fn parse_tag_string(tag_str: &Option<String>) -> Option<HashSet<Tag>> {
    tag_str.as_ref().and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Tag::parse_tags(s).ok()
        }
    })
}

// Apply prefix tags to base tags, returning a new set with all tags
#[instrument(level = "trace")]
pub fn apply_prefix_tags(
    base_tags: Option<HashSet<Tag>>,
    prefix_tags: Option<HashSet<Tag>>,
) -> Option<HashSet<Tag>> {
    match (base_tags, prefix_tags) {
        (None, None) => None,
        (Some(base), None) => Some(base),
        (None, Some(prefix)) => Some(prefix),
        (Some(mut base), Some(prefix)) => {
            base.extend(prefix);
            Some(base)
        }
    }
}

// Determine sort direction based on order flags
#[instrument(level = "trace")]
fn determine_sort_direction(order_desc: bool, order_asc: bool) -> SortDirection {
    match (order_desc, order_asc) {
        (true, false) => SortDirection::Descending,
        (false, true) => SortDirection::Ascending,
        _ => SortDirection::Descending, // Default to descending
    }
}

#[instrument(skip(stderr, cli))]
pub fn search(mut stderr: StandardStream, cli: Cli) -> CliResult<()> {
    // Extract all arguments from the Search command
    if let Commands::Search {
        fts_query,
        tags_exact,
        tags_exact_prefix,
        tags_all,
        tags_all_prefix,
        tags_all_not,
        tags_all_not_prefix,
        tags_any,
        tags_any_prefix,
        tags_any_not,
        tags_any_not_prefix,
        order_desc,
        order_asc,
        non_interactive,
        is_fuzzy,
        fzf_style,
        is_json,
        limit,
    } = cli.command.unwrap()
    {
        let mut fields = DEFAULT_FIELDS.to_vec();

        // Get service
        let service = create_bookmark_service();

        // Determine sort direction
        let sort_direction = determine_sort_direction(order_desc, order_asc);

        if order_desc || order_asc {
            fields.push(DisplayField::LastUpdateTs);
        }

        // Parse all tag sets and apply prefixes
        let exact_tags = apply_prefix_tags(
            parse_tag_string(&tags_exact),
            parse_tag_string(&tags_exact_prefix),
        );

        let all_tags = apply_prefix_tags(
            parse_tag_string(&tags_all),
            parse_tag_string(&tags_all_prefix),
        );

        let all_not_tags = apply_prefix_tags(
            parse_tag_string(&tags_all_not),
            parse_tag_string(&tags_all_not_prefix),
        );

        let any_tags = apply_prefix_tags(
            parse_tag_string(&tags_any),
            parse_tag_string(&tags_any_prefix),
        );

        let any_not_tags = apply_prefix_tags(
            parse_tag_string(&tags_any_not),
            parse_tag_string(&tags_any_not_prefix),
        );

        // Use the service to perform the search
        let bookmarks = service.search_bookmarks(
            fts_query.as_deref(),
            exact_tags.as_ref(),
            all_tags.as_ref(),
            all_not_tags.as_ref(),
            any_tags.as_ref(),
            any_not_tags.as_ref(),
            None, // We don't use tags_prefix in the service call anymore
            sort_direction,
            limit.map(|v| v as usize),
        )?;

        // Handle different output modes
        match (is_fuzzy, is_json) {
            (true, _) => {
                let style = fzf_style.as_deref().unwrap_or("classic");
                fzf_process(&bookmarks, style)?;
            }
            (_, true) => {
                let json_views = JsonBookmarkView::from_domain_collection(&bookmarks);
                write_bookmarks_as_json(&json_views)?;
            }
            _ => {
                display_search_results(&mut stderr, &bookmarks, &fields, non_interactive)?;
            }
        }
    }
    Ok(())
}

// Function to display search results in normal mode
#[instrument(skip(stderr, bookmarks, fields), level = "debug")]
fn display_search_results(
    stderr: &mut StandardStream,
    bookmarks: &[Bookmark],
    fields: &[DisplayField],
    non_interactive: bool,
) -> CliResult<()> {
    // Convert to display bookmarks
    let display_bookmarks: Vec<DisplayBookmark> =
        bookmarks.iter().map(DisplayBookmark::from_domain).collect();

    show_bookmarks(&display_bookmarks, fields);
    eprintln!("Found {} bookmarks", bookmarks.len());

    if non_interactive {
        let ids = bookmarks
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
        writeln!(stderr, "Selection: ")
            .map_err(|e| CliError::Other(format!("Failed to write to stderr: {}", e)))?;
        stderr
            .reset()
            .map_err(|e| CliError::Other(format!("Failed to reset color: {}", e)))?;

        process(bookmarks)?;
    }
    Ok(())
}

#[instrument(skip(stderr, cli))]
pub fn semantic_search(mut stderr: StandardStream, cli: Cli) -> CliResult<()> {
    if let Commands::SemSearch {
        query,
        limit,
        non_interactive,
    } = cli.command.unwrap()
    {
        let bookmark_service = create_bookmark_service();

        // Create the semantic search domain object
        let search = SemanticSearch::new(query, limit.map(|l| l as usize));

        // Perform semantic search
        let results = bookmark_service.semantic_search(&search)?;

        if results.is_empty() {
            writeln!(stderr, "{}", "No bookmarks found".yellow())?;
            return Ok(());
        }

        // Format and display results with similarity scores
        for result in &results {
            writeln!(
                stderr,
                "{} {} [{}] ({})",
                result
                    .bookmark
                    .id
                    .map_or("?".to_string(), |id| id.to_string())
                    .blue(),
                result.bookmark.title.clone().green(),
                result.bookmark.formatted_tags().yellow(),
                result.similarity_percentage().cyan()
            )?;
            writeln!(stderr, "  {}", result.bookmark.url)?;
            if !result.bookmark.description.is_empty() {
                writeln!(stderr, "  {}", result.bookmark.description)?;
            }
            writeln!(stderr)?;
        }

        writeln!(stderr, "{} bookmarks found", results.len())?;

        // In interactive mode, prompt for actions
        if !non_interactive && !results.is_empty() && confirm("Open bookmark(s)?") {
            // Prompt for which bookmark to open
            print!("Enter ID(s) to open (comma-separated): ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let ids = get_ids(input.trim().to_string())?;
            for id in ids {
                if let Some(result) = results.iter().find(|r| r.bookmark.id == Some(id)) {
                    open_bookmark(&result.bookmark)?;
                } else {
                    writeln!(stderr, "Bookmark with ID {} not found in results", id)?;
                }
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn open(cli: Cli) -> CliResult<()> {
    if let Commands::Open { ids } = cli.command.unwrap() {
        let bookmark_service = create_bookmark_service();

        for id in get_ids(ids)? {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                // Use open_bookmark instead of direct open_url to ensure access is recorded
                open_bookmark(&bookmark)?;
                println!("Opened: {}", bookmark.url);
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

// TODO: simplify redundant code
#[instrument(skip(cli))]
pub fn add(cli: Cli) -> CliResult<()> {
    if let Commands::Add {
        url,
        tags,
        title,
        desc,
        no_web,
        edit,
        bookmark_type,
    } = cli.command.unwrap()
    {
        let bookmark_service = create_bookmark_service();
        let tag_service = factory::create_tag_service();
        let template_service = factory::create_template_service();

        // Convert bookmark_type string to SystemTag
        let system_tag = match bookmark_type.to_lowercase().as_str() {
            "snip" => SystemTag::Snippet,
            "text" => SystemTag::Text,
            _ => SystemTag::Uri, // Default to Uri for anything else
        };

        // Parse tags if provided
        let mut tag_set = if let Some(tag_str) = tags {
            let parsed_tags = tag_service.parse_tag_string(&tag_str)?;
            parsed_tags.into_iter().collect::<HashSet<_>>()
        } else {
            HashSet::new()
        };

        // Add the system tag if it's not Uri (which has an empty string as_str)
        if system_tag != SystemTag::Uri {
            if let Ok(system_tag_value) = Tag::new(system_tag.as_str()) {
                tag_set.insert(system_tag_value);
            }
        }

        // If URL is not provided or edit flag is set, open the editor
        if url.is_none() || edit {
            // Create a template for the specific bookmark type
            let mut template = BookmarkTemplate::for_type(system_tag);

            // Override with provided values if they exist
            if let Some(url_value) = &url {
                template.url = url_value.clone();
            }
            if let Some(title_value) = &title {
                template.title = title_value.clone();
            }
            if let Some(desc_value) = &desc {
                template.comments = desc_value.clone();
            }

            // Add user-provided tags
            for tag in &tag_set {
                template.tags.insert(tag.clone());
            }

            // Convert the template to a bookmark
            let temp_bookmark = template.to_bookmark(None).map_err(|e| {
                CliError::Other(format!("Failed to create temporary bookmark: {}", e))
            })?;

            // Open the editor with our prepared template
            match template_service.edit_bookmark_with_template(Some(temp_bookmark)) {
                Ok((edited_bookmark, was_modified)) => {
                    if !was_modified {
                        println!("No changes made in editor. Bookmark not added.");
                        return Ok(());
                    }

                    // Add the edited bookmark
                    match bookmark_service.add_bookmark(
                        &edited_bookmark.url,
                        Some(&edited_bookmark.title),
                        Some(&edited_bookmark.description),
                        Some(&edited_bookmark.tags),
                        false, // Don't fetch metadata since we've already edited it
                    ) {
                        Ok(bookmark) => {
                            println!(
                                "Added bookmark: {} (ID: {})",
                                bookmark.title,
                                bookmark.id.unwrap_or(0)
                            );
                        }
                        Err(e) => {
                            return Err(CliError::CommandFailed(format!(
                                "Failed to add bookmark: {}",
                                e
                            )));
                        }
                    }
                }
                Err(e) => {
                    return Err(CliError::CommandFailed(format!(
                        "Failed to edit bookmark: {}",
                        e
                    )));
                }
            }
        } else {
            // Regular add without editing - URL must be provided in this case
            let url = url.unwrap();
            let bookmark = bookmark_service.add_bookmark(
                &url,
                title.as_deref(),
                desc.as_deref(),
                Some(&tag_set),
                !no_web,
            )?;

            println!(
                "Added bookmark: {} (ID: {})",
                bookmark.title,
                bookmark.id.unwrap_or(0)
            );
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn delete(cli: Cli) -> CliResult<()> {
    if let Commands::Delete { ids } = cli.command.unwrap() {
        let bookmark_service = create_bookmark_service();

        let id_list = get_ids(ids)?;

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                println!("Deleting: {} ({})", bookmark.title, bookmark.url);

                if confirm("Confirm delete?") {
                    match bookmark_service.delete_bookmark(id) {
                        Ok(true) => println!("Deleted bookmark with ID {}", id),
                        Ok(false) => println!("Bookmark with ID {} not found", id),
                        Err(e) => println!("Error deleting bookmark with ID {}: {}", id, e),
                    }
                } else {
                    println!("Deletion cancelled");
                }
            } else {
                println!("Bookmark with ID {} not found", id);
            }
        }
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
        let bookmark_service = create_bookmark_service();
        let tag_service = factory::create_tag_service();

        let id_list = get_ids(ids)?;

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                println!("Updating: {} ({})", bookmark.title, bookmark.url);

                if force && tags.is_some() {
                    // Replace all tags
                    let tag_str = tags.as_ref().unwrap();
                    let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                    let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                    let updated = bookmark_service.replace_bookmark_tags(id, &tag_set)?;
                    println!("Tags replaced: {}", updated.formatted_tags());
                } else {
                    // Add tags if provided
                    if let Some(tag_str) = &tags {
                        let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                        let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                        let updated = bookmark_service.add_tags_to_bookmark(id, &tag_set)?;
                        println!("Tags added: {}", updated.formatted_tags());
                    }

                    // Remove tags if provided
                    if let Some(tag_str) = &tags_not {
                        let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                        let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                        let updated = bookmark_service.remove_tags_from_bookmark(id, &tag_set)?;
                        println!("Tags removed: {}", updated.formatted_tags());
                    }
                }
            } else {
                println!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn edit(cli: Cli) -> CliResult<()> {
    if let Commands::Edit { ids } = cli.command.unwrap() {
        let bookmark_service = create_bookmark_service();

        let id_list = get_ids(ids)?;

        // Get all bookmarks to edit first
        let mut bookmarks_to_edit = Vec::new();
        for id in &id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(*id)? {
                bookmarks_to_edit.push(bookmark);
            } else {
                println!("Bookmark with ID {} not found", id);
            }
        }

        if bookmarks_to_edit.is_empty() {
            println!("No bookmarks found to edit");
            return Ok(());
        }

        edit_bookmarks(id_list)?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn show(cli: Cli) -> CliResult<()> {
    if let Commands::Show { ids } = cli.command.unwrap() {
        let bookmark_service = create_bookmark_service();

        let id_list = get_ids(ids)?;

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                println!(
                    "{} {} [{}]",
                    bookmark
                        .id
                        .map_or("?".to_string(), |id| id.to_string())
                        .blue(),
                    bookmark.title.clone().green(),
                    bookmark.formatted_tags().yellow()
                );
                println!("  URL: {}", bookmark.url);
                println!("  Description: {}", bookmark.description);
                println!("  Access count: {}", bookmark.access_count);
                println!("  Created: {}", bookmark.created_at);
                println!("  Updated: {}", bookmark.updated_at);
                println!("  Has embedding: {}", bookmark.embedding.is_some());
                println!();
            } else {
                println!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn surprise(cli: Cli) -> CliResult<()> {
    if let Commands::Surprise { n } = cli.command.unwrap() {
        let bookmark_service = create_bookmark_service();

        // Get random bookmarks
        let count = if n < 1 { 1 } else { n as usize };
        let bookmarks = bookmark_service.get_random_bookmarks(count)?;

        if bookmarks.is_empty() {
            println!("No bookmarks found");
            return Ok(());
        }

        println!("Opening {} random bookmarks:", bookmarks.len());

        for bookmark in &bookmarks {
            // Use open_bookmark to ensure access is recorded
            println!("Opening: {} ({})", bookmark.title, bookmark.url);
            open_bookmark(bookmark)?;
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn create_db(cli: Cli) -> CliResult<()> {
    if let Commands::CreateDb { path } = cli.command.unwrap() {
        // Check if the database file already exists
        if Path::new(&path).exists() {
            return Err(CliError::InvalidInput(format!(
                "Database already exists at: {}. Please choose a different path or delete the existing file.",
                path
            )));
        }

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    CliError::Io(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to create parent directories: {}", e),
                    ))
                })?;
            }
        }

        println!("Creating new database at: {}", path);

        // Create the repository with the new path
        let repository = SqliteBookmarkRepository::from_url(&path)?;

        // Get a connection
        let mut conn = repository.get_connection()?;

        // Run migrations to set up the schema
        migration::init_db(&mut conn)?;

        // Clean the bookmark table to ensure we start with an empty database
        repository.empty_bookmark_table()?;

        println!("Database created successfully at: {}", path);
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn set_embeddable(cli: Cli) -> CliResult<()> {
    if let Commands::SetEmbeddable {
        id,
        enable,
        disable,
    } = cli.command.unwrap()
    {
        let bookmark_service = create_bookmark_service();

        // Ensure that exactly one flag is provided
        if enable == disable {
            return Err(CliError::InvalidInput(
                "Exactly one of --enable or --disable must be specified".to_string(),
            ));
        }

        // Set the embeddable flag
        let embeddable = enable;
        match bookmark_service.set_bookmark_embeddable(id, embeddable) {
            Ok(bookmark) => {
                println!(
                    "Bookmark '{}' (ID: {}) is now {} for embedding",
                    bookmark.title,
                    bookmark.id.unwrap_or(0),
                    if embeddable { "enabled" } else { "disabled" }
                );
                Ok(())
            }
            Err(e) => Err(CliError::from(e)),
        }
    } else {
        Err(CliError::Other("Invalid command".to_string()))
    }
}

#[instrument(skip(cli))]
pub fn backfill(cli: Cli) -> CliResult<()> {
    if let Commands::Backfill { dry_run } = cli.command.unwrap() {
        let app_state = AppState::read_global();
        if app_state.context.embedder.as_any().type_id() == std::any::TypeId::of::<DummyEmbedding>()
        {
            eprintln!("{}", "Error: Cannot backfill embeddings with DummyEmbedding active. Please use --openai flag.".red());
            return Err(CliError::CommandFailed(
                "DummyEmbedding active - embeddings not available".to_string(),
            ));
        }
        let bookmark_service = create_bookmark_service();

        // Get bookmarks without embeddings but with embeddable=true
        let bookmarks = bookmark_service.get_bookmarks_without_embeddings()?;

        if bookmarks.is_empty() {
            println!("No embeddable bookmarks found that need embeddings");
            return Ok(());
        }

        println!(
            "Found {} embeddable bookmarks without embeddings",
            bookmarks.len()
        );

        if dry_run {
            // Just show the bookmarks that would be processed
            for bookmark in &bookmarks {
                println!(
                    "Would update: {} (ID: {})",
                    bookmark.title,
                    bookmark.id.unwrap_or(0)
                );
            }
        } else {
            // Process each bookmark
            for bookmark in &bookmarks {
                if let Some(id) = bookmark.id {
                    println!("Updating embedding for: {} (ID: {})", bookmark.title, id);
                    match bookmark_service.update_bookmark(bookmark.clone()) {
                        Ok(_) => println!("  Successfully updated embedding"),
                        Err(e) => println!("  Failed to update embedding: {}", e),
                    }
                }
            }
            println!(
                "Completed embedding backfill for {} bookmarks",
                bookmarks.len()
            );
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn load_json(cli: Cli) -> CliResult<()> {
    if let Commands::LoadJson { path, dry_run } = cli.command.unwrap() {
        println!("Loading bookmarks from JSON array: {}", path);

        let bookmark_service = create_bookmark_service();

        if dry_run {
            let count = bookmark_service.load_json_bookmarks(&path, true)?;
            println!(
                "Dry run completed - would process {} bookmark entries",
                count
            );
            return Ok(());
        }

        // Process the bookmarks
        let processed_count = bookmark_service.load_json_bookmarks(&path, false)?;
        println!(
            "Successfully processed {} bookmark entries",
            processed_count
        );
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn load_texts(cli: Cli) -> CliResult<()> {
    if let Commands::LoadTexts { dry_run, path } = cli.command.unwrap() {
        let app_state = AppState::read_global();
        if app_state.context.embedder.as_any().type_id() == std::any::TypeId::of::<DummyEmbedding>()
        {
            eprintln!(
                "{}",
                "Error: Cannot load texts with DummyEmbedding active. Please use --openai flag."
                    .red()
            );
            return Err(CliError::CommandFailed(
                "DummyEmbedding active - embeddings not available".to_string(),
            ));
        }

        println!("Loading text documents from NDJSON file: {}", path);
        println!("(Expecting one JSON document per line)");

        let bookmark_service = create_bookmark_service();

        if dry_run {
            let count = bookmark_service.load_texts(&path, true)?;
            println!("Dry run completed - would process {} text entries", count);
            return Ok(());
        }

        // Process the texts
        let processed_count = bookmark_service.load_texts(&path, false)?;
        println!("Successfully processed {} text entries", processed_count);
    }
    Ok(())
}

// src/cli/bookmark_commands.rs

#[instrument(skip(cli))]
pub fn info(cli: Cli) -> CliResult<()> {
    if let Commands::Info { show_schema } = cli.command.unwrap() {
        let app_state = AppState::read_global();
        let repository = SqliteBookmarkRepository::from_url(&app_state.settings.db_url)?;

        // Program version
        println!("Program Version: {}", env!("CARGO_PKG_VERSION"));

        // App configuration
        println!("\nConfiguration:");
        println!("  Database URL: {}", app_state.settings.db_url);
        println!("  FZF Height: {}", app_state.settings.fzf_opts.height);
        println!("  FZF Reverse: {}", app_state.settings.fzf_opts.reverse);
        println!("  FZF Show Tags: {}", app_state.settings.fzf_opts.show_tags);
        println!("  FZF Hide URL: {}", app_state.settings.fzf_opts.no_url);

        // Embedder type
        let embedder_type = if app_state.context.embedder.as_any().type_id()
            == std::any::TypeId::of::<DummyEmbedding>()
        {
            "DummyEmbedding (embeddings disabled)"
        } else {
            "OpenAiEmbedding (embeddings enabled)"
        };
        println!("  Embedder: {}", embedder_type);

        // Number of entries
        let bookmark_count = repository.get_all()?.len();
        println!("\nDatabase Statistics:");
        println!("  Total Bookmarks: {}", bookmark_count);

        // Get tag statistics
        let tags = repository.get_all_tags()?;
        println!("  Total Tags: {}", tags.len());
        println!("  Top 5 Tags:");
        for (tag, count) in tags.iter().take(5) {
            println!("    {} ({})", tag.value(), count);
        }

        // Show schema if requested
        if show_schema {
            println!("\nDatabase Schema:");
            print_db_schema(&repository);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ids_valid() {
        let ids = "1,2,3,4,5".to_string();
        let result = get_ids(ids);
        assert!(result.is_ok());
        let id_list = result.unwrap();
        assert_eq!(id_list, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_get_ids_invalid() {
        let ids = "1,2,three,4,5".to_string();
        let result = get_ids(ids);
        assert!(result.is_err());
    }

    #[test]
    fn given_valid_tag_string_when_parse_tag_string_then_returns_tag_set() {
        // Arrange
        let tag_str = Some("tag1,tag2,tag3".to_string());

        // Act
        let result = parse_tag_string(&tag_str);

        // Assert
        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));
    }

    #[test]
    fn given_empty_tag_string_when_parse_tag_string_then_returns_none() {
        // Arrange
        let tag_str = Some("".to_string());

        // Act
        let result = parse_tag_string(&tag_str);

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn given_none_tag_string_when_parse_tag_string_then_returns_none() {
        // Arrange
        let tag_str: Option<String> = None;

        // Act
        let result = parse_tag_string(&tag_str);

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn given_base_and_prefix_when_apply_prefix_tags_then_returns_combined() {
        // Arrange
        let mut base_tags = HashSet::new();
        base_tags.insert(Tag::new("base1").unwrap());
        base_tags.insert(Tag::new("base2").unwrap());

        let mut prefix_tags = HashSet::new();
        prefix_tags.insert(Tag::new("prefix1").unwrap());
        prefix_tags.insert(Tag::new("prefix2").unwrap());

        // Act
        let result = apply_prefix_tags(Some(base_tags), Some(prefix_tags));

        // Assert
        assert!(result.is_some());
        let combined = result.unwrap();
        assert_eq!(combined.len(), 4);
        assert!(combined.contains(&Tag::new("base1").unwrap()));
        assert!(combined.contains(&Tag::new("base2").unwrap()));
        assert!(combined.contains(&Tag::new("prefix1").unwrap()));
        assert!(combined.contains(&Tag::new("prefix2").unwrap()));
    }

    #[test]
    fn given_only_base_when_apply_prefix_tags_then_returns_base() {
        // Arrange
        let mut base_tags = HashSet::new();
        base_tags.insert(Tag::new("base1").unwrap());
        base_tags.insert(Tag::new("base2").unwrap());

        // Act
        let result = apply_prefix_tags(Some(base_tags), None);

        // Assert
        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("base1").unwrap()));
        assert!(tags.contains(&Tag::new("base2").unwrap()));
    }

    #[test]
    fn given_only_prefix_when_apply_prefix_tags_then_returns_prefix() {
        // Arrange
        let mut prefix_tags = HashSet::new();
        prefix_tags.insert(Tag::new("prefix1").unwrap());
        prefix_tags.insert(Tag::new("prefix2").unwrap());

        // Act
        let result = apply_prefix_tags(None, Some(prefix_tags));

        // Assert
        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("prefix1").unwrap()));
        assert!(tags.contains(&Tag::new("prefix2").unwrap()));
    }

    #[test]
    fn given_none_for_both_when_apply_prefix_tags_then_returns_none() {
        // Act
        let result = apply_prefix_tags(None, None);

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn given_desc_flag_when_determine_sort_direction_then_returns_descending() {
        // Act
        let result = determine_sort_direction(true, false);

        // Assert
        assert_eq!(result, SortDirection::Descending);
    }

    #[test]
    fn given_asc_flag_when_determine_sort_direction_then_returns_ascending() {
        // Act
        let result = determine_sort_direction(false, true);

        // Assert
        assert_eq!(result, SortDirection::Ascending);
    }

    #[test]
    fn given_both_flags_when_determine_sort_direction_then_returns_descending() {
        // Act
        let result = determine_sort_direction(true, true);

        // Assert
        assert_eq!(result, SortDirection::Descending);
    }

    #[test]
    fn given_no_flags_when_determine_sort_direction_then_returns_descending() {
        // Act
        let result = determine_sort_direction(false, false);

        // Assert
        assert_eq!(result, SortDirection::Descending);
    }
}
