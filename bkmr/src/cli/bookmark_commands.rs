// src/cli/bookmark_commands.rs
use crate::config::Settings;
use crate::application::actions::MarkdownAction;
// Service container dependency injection implemented via SearchCommandHandler
use crate::infrastructure::di::ServiceContainer;
use crate::application::templates::bookmark_template::BookmarkTemplate;
use crate::cli::args::{Cli, Commands};
use crate::cli::error::{CliError, CliResult};
use crate::cli::process::{edit_bookmarks, execute_bookmark_default_action};
use crate::config::ConfigSource;
use crate::domain::bookmark::Bookmark;
use crate::domain::error_context::CliErrorContext;
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
use crate::util::argument_processor::ArgumentProcessor;
use crate::util::helper::{confirm, ensure_int_vector, is_stdout_piped};
use crate::util::helper::{format_file_path, format_mtime};
use chrono;
use crossterm::style::Stylize;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::{fs, io};
use termcolor::StandardStream;
use tracing::{instrument, warn};

// Helper function to get and validate IDs
fn get_ids(ids: String) -> CliResult<Vec<i32>> {
    // ensure_int_vector(&ids.split(',').map(String::from).collect())
    //     .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
    let string_vec: Vec<String> = ids.split(',').map(|s| s.trim().to_string()).collect();
    ensure_int_vector(&string_vec)
        .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
}

#[instrument(skip(stderr, cli))]
pub fn semantic_search(mut stderr: StandardStream, cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::SemSearch {
        query,
        limit,
        non_interactive,
    } = cli.command.unwrap()
    {
        let bookmark_service = services.bookmark_service.clone();

        // Create the semantic search domain object
        let search = SemanticSearch::new(query, limit.map(|l| l as usize));

        // Perform semantic search
        let results = bookmark_service.semantic_search(&search)?;

        if results.is_empty() {
            writeln!(stderr, "{}", "No bookmarks found".yellow())?;
            return Ok(());
        }

        let is_piped = is_stdout_piped();

        if is_piped {
            // Generate plain-text output for piping
            for result in &results {
                println!(
                    "{}\t{}\t{}\t{}",
                    result.bookmark.id.unwrap_or(0),
                    result.bookmark.title,
                    result.bookmark.url,
                    result.similarity_percentage()
                );
            }
        } else {
            // Format and display results with similarity scores for interactive use
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
        }

        writeln!(stderr, "{} bookmarks found", results.len())?;

        // In interactive mode, prompt for actions
        if !non_interactive && !is_piped && !results.is_empty() && confirm("Open bookmark(s)?") {
            // Prompt for which bookmark to open
            print!("Enter ID(s) to open (comma-separated): ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let ids = get_ids(input.trim().to_string())?;
            for id in ids {
                if let Some(result) = results.iter().find(|r| r.bookmark.id == Some(id)) {
                    execute_bookmark_default_action(&result.bookmark, services)?;
                } else {
                    writeln!(stderr, "Bookmark with ID {} not found in results", id)?;
                }
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn open(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Open {
        ids,
        no_edit,
        file,
        script_args,
    } = cli.command.unwrap()
    {
        if file {
            // Handle direct file viewing
            handle_file_viewing(&ids)?;
        } else {
            // Handle bookmark opening (existing logic)
            let bookmark_service = services.bookmark_service.clone();
            let action_service = services.action_service.clone();

            for id in get_ids(ids)? {
                if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                    // Use action service to execute default action
                    let action_type = action_service.get_default_action_description(&bookmark);
                    eprintln!("Performing '{}' for: {}", action_type, bookmark.title);

                    // Execute default action with access recording handled by action service
                    action_service.execute_default_action_with_options(
                        &bookmark,
                        no_edit,
                        &script_args,
                    )?;
                } else {
                    eprintln!("Bookmark with ID {} not found", id);
                }
            }
        }
    }
    Ok(())
}

/// Handle direct file viewing by creating a temporary bookmark and using existing markdown functionality
fn handle_file_viewing(file_path: &str) -> CliResult<()> {
    use crate::domain::action::BookmarkAction;
    use std::collections::HashSet;

    // Check if file exists first
    if !Path::new(file_path).exists() {
        return Err(CliError::Other(format!(
            "File not found: {}. Please check the path and try again.",
            file_path
        )));
    }

    // Extract filename for display
    let path = Path::new(file_path);
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_path);

    eprintln!("Rendering file: {}", filename);

    // Create markdown action with dummy embedder (reuses existing file reading logic)
    let embedder = Arc::new(DummyEmbedding);
    let markdown_action = MarkdownAction::new(embedder);

    // Create a minimal bookmark structure - the MarkdownAction already handles file paths in url field
    let mut tags = HashSet::new();
    tags.insert(Tag::new("_md_").map_err(|e| CliError::Other(e.to_string()))?);

    let temp_bookmark = Bookmark {
        id: None,
        url: file_path.to_string(), // File path - MarkdownAction will detect this and read the file
        title: filename.to_string(),
        description: format!("Direct file view: {}", file_path),
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

    // Execute the markdown action - it will handle file reading and rendering
    markdown_action
        .execute(&temp_bookmark)
        .cli_context("Failed to render file")?;

    Ok(())
}

/// Helper function to process content based on bookmark type
fn process_content_for_type(content: &str, system_tag: SystemTag) -> String {
    if matches!(
        system_tag,
        SystemTag::Markdown
            | SystemTag::Snippet
            | SystemTag::Text
            | SystemTag::Shell
            | SystemTag::Env
    ) {
        content.replace("\\n", "\n")
    } else {
        content.to_string()
    }
}

#[instrument(skip(cli))]
pub fn add(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Add {
        url,
        tags,
        title,
        desc,
        no_web,
        edit,
        bookmark_type,
        clone_id,
        stdin,
    } = cli.command.unwrap()
    {
        let bookmark_service = &services.bookmark_service;
        let template_service = &services.template_service;

        // Convert bookmark_type string to SystemTag
        let system_tag = match bookmark_type.to_lowercase().as_str() {
            "snip" => SystemTag::Snippet,
            "text" => SystemTag::Text,
            "shell" => SystemTag::Shell,
            "md" | "markdown" => SystemTag::Markdown,
            "env" => SystemTag::Env,
            _ => SystemTag::Uri, // Default to Uri for anything else
        };

        // Parse tags using centralized argument processor
        let mut tag_set = ArgumentProcessor::parse_tags_with_error_handling(&tags)?;
        // Add the system tag if it's not Uri (which has an empty string as_str)
        if system_tag != SystemTag::Uri {
            if let Ok(system_tag_value) = Tag::new(system_tag.as_str()) {
                tag_set.insert(system_tag_value);
            }
        }

        // Handle stdin input - read content from stdin if requested
        let final_url = if stdin {
            use std::io::{self, Read};
            let mut content = String::new();
            io::stdin()
                .read_to_string(&mut content)
                .cli_context("Failed to read from stdin")?;

            // Trim trailing newline for cleaner content
            Some(content.trim_end().to_string())
        } else {
            url
        };

        // Prepare the template - either from clone or new
        let mut template = if let Some(id) = clone_id {
            // Get the bookmark to clone
            let bookmark = bookmark_service.get_bookmark(id)?.ok_or_else(|| {
                CliError::InvalidInput(format!("No bookmark found with ID {}", id))
            })?;

            // Create a template with the bookmark data but without ID
            let mut template = BookmarkTemplate::from_bookmark(&bookmark);
            template.id = None; // Clear ID to ensure a new bookmark will be created
            template
        } else {
            // Create a new template for the specific bookmark type
            BookmarkTemplate::for_type(system_tag)
        };

        // Override with provided values if they exist
        if let Some(url_value) = &final_url {
            template.url = process_content_for_type(url_value, system_tag);
        }

        if let Some(title_value) = &title {
            template.title = title_value.clone();
        }
        if let Some(desc_value) = &desc {
            template.comments = desc_value.clone();
        }

        // Add user-provided tags (keep existing tags for cloned bookmarks)
        for tag in &tag_set {
            template.tags.insert(tag.clone());
        }

        // If URL is provided, edit flag is not set, and we're not cloning,
        // use the simple add path without opening editor
        if final_url.is_some() && !edit && clone_id.is_none() {
            let url_value = final_url.unwrap();
            let processed_content = process_content_for_type(&url_value, system_tag);

            let bookmark = bookmark_service.add_bookmark(
                &processed_content,
                title.as_deref(),
                desc.as_deref(),
                Some(&tag_set),
                !no_web,
            )?;

            eprintln!(
                "Added bookmark: {} (ID: {})",
                bookmark.title,
                bookmark.id.unwrap_or(0)
            );
            return Ok(());
        }

        // Otherwise, open in editor
        // Convert template to a temporary bookmark for editing
        let temp_bookmark = template
            .to_bookmark(None)
            .map_err(|e| CliError::Other(format!("Failed to create temporary bookmark: {}", e)))?;

        // Open the editor with our prepared template
        match template_service.edit_bookmark_with_template(Some(temp_bookmark)) {
            Ok((edited_bookmark, was_modified)) => {
                if !was_modified {
                    eprintln!("No changes made in editor. Bookmark not added.");
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
                        eprintln!(
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
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn delete(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Delete { ids } = cli.command.unwrap() {
        let bookmark_service = services.bookmark_service.clone();

        let id_list = get_ids(ids)?;

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                eprintln!("Deleting: {} ({})", bookmark.title, bookmark.url);

                if confirm("Confirm delete?") {
                    match bookmark_service.delete_bookmark(id) {
                        Ok(true) => eprintln!("Deleted bookmark with ID {}", id),
                        Ok(false) => eprintln!("Bookmark with ID {} not found", id),
                        Err(e) => eprintln!("Error deleting bookmark with ID {}: {}", id, e),
                    }
                } else {
                    eprintln!("Deletion cancelled");
                }
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn update(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Update {
        ids,
        tags,
        tags_not,
        force,
    } = cli.command.unwrap()
    {
        let bookmark_service = services.bookmark_service.clone();
        let tag_service = services.tag_service.clone();

        let id_list = get_ids(ids)?;

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                eprintln!("Updating: {} ({})", bookmark.title, bookmark.url);

                if force && tags.is_some() {
                    // Replace all tags
                    let tag_str = tags.as_ref().unwrap();
                    let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                    let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                    let updated = bookmark_service.replace_bookmark_tags(id, &tag_set)?;
                    eprintln!("Tags replaced: {}", updated.formatted_tags());
                } else {
                    // Add tags if provided
                    if let Some(tag_str) = &tags {
                        let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                        let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                        let updated = bookmark_service.add_tags_to_bookmark(id, &tag_set)?;
                        eprintln!("Tags added: {}", updated.formatted_tags());
                    }

                    // Remove tags if provided
                    if let Some(tag_str) = &tags_not {
                        let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                        let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                        let updated = bookmark_service.remove_tags_from_bookmark(id, &tag_set)?;
                        eprintln!("Tags removed: {}", updated.formatted_tags());
                    }
                }
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn edit(cli: Cli, services: &ServiceContainer, settings: &crate::config::Settings) -> CliResult<()> {
    if let Commands::Edit { ids, force_db } = cli.command.unwrap() {
        let bookmark_service = services.bookmark_service.clone();

        let id_list = get_ids(ids)?;

        // Get all bookmarks to edit first
        let mut bookmarks_to_edit = Vec::new();
        for id in &id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(*id)? {
                bookmarks_to_edit.push(bookmark);
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }

        if bookmarks_to_edit.is_empty() {
            eprintln!("No bookmarks found to edit");
            return Ok(());
        }

        edit_bookmarks(id_list, force_db, services, settings)?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn show(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Show { ids, is_json } = cli.command.unwrap() {
        let bookmark_service = services.bookmark_service.clone();

        let id_list = get_ids(ids)?;
        let mut bookmarks = Vec::new();

        for id in id_list {
            if let Some(_bookmark) = bookmark_service.get_bookmark(id)? {
                let updated_bookmark = bookmark_service.record_bookmark_access(id)?;
                bookmarks.push(updated_bookmark);
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }

        if is_json {
            // Output bookmarks as JSON
            let json_views = JsonBookmarkView::from_domain_collection(&bookmarks);
            write_bookmarks_as_json(&json_views)?;
        } else {
            // Output bookmarks in detailed format (existing behavior)
            for bookmark in &bookmarks {
                print!("{}", show_bookmark_details(bookmark, services));
            }
        }
    }
    Ok(())
}

/// Shows detailed information about a bookmark, can be used by both show command and FZF CTRL-P
#[instrument(level = "debug")]
pub fn show_bookmark_details(bookmark: &Bookmark, services: &ServiceContainer) -> String {
    // Get the action service
    let action_service = services.action_service.clone();

    // Get the action description
    let action_description = action_service.get_default_action_description(bookmark);

    // Format all the bookmark details
    let mut details = format!(
        "{} {} [{}] ({})\n  URL/Content: {}\n  Description: {}\n  Access count: {}\n  Created: {}\n  Updated: {}\n  Has embedding: {}\n  Default Action: {}",
        bookmark
            .id
            .map_or("?".to_string(), |id| id.to_string())
            .blue(),
        bookmark.title.clone().green(),
        bookmark.formatted_tags().yellow(),
        action_description.cyan(),
        bookmark.url,
        bookmark.description,
        bookmark.access_count,
        bookmark.created_at.map_or("N/A".to_string(), |dt| dt.to_string()),
        bookmark.updated_at,
        bookmark.embedding.is_some(),
        action_description
    );

    // Add file info if present
    if let (Some(file_path), Some(file_mtime)) = (&bookmark.file_path, bookmark.file_mtime) {
        let formatted_path = format_file_path(file_path, 120);
        let formatted_time = format_mtime(file_mtime);
        details.push_str(&format!(
            "\n  Source File: {} (modified: {})",
            formatted_path, formatted_time
        ));
    }

    details.push('\n');
    details
}

#[instrument(skip(cli))]
pub fn surprise(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Surprise { n } = cli.command.unwrap() {
        let bookmark_service = services.bookmark_service.clone();
        let action_service = services.action_service.clone();

        // Get random bookmarks
        let count = if n < 1 { 1 } else { n as usize };
        let bookmarks = bookmark_service.get_random_bookmarks(count)?;

        if bookmarks.is_empty() {
            eprintln!("No bookmarks found");
            return Ok(());
        }

        eprintln!("Processing {} random bookmarks:", bookmarks.len());

        for bookmark in &bookmarks {
            // Get the action description
            let action_description = action_service.get_default_action_description(bookmark);

            // Show what we're doing
            eprintln!(
                "Performing '{}' for: {} ({})",
                action_description, bookmark.title, bookmark.url
            );

            // Execute the default action
            action_service.execute_default_action(bookmark)?;
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn create_db(cli: Cli, services: &ServiceContainer, settings: &Settings) -> CliResult<()> {
    if let Commands::CreateDb { path, pre_fill } = cli.command.unwrap() {
        // Get the database path from either the command-line argument or the config system
        let db_path = match path {
            Some(p) => p,
            None => {
                // Get from config system via settings parameter
                let configured_path = &settings.db_url;

                // Check if we're using default configuration
                if settings.config_source == ConfigSource::Default {
                    eprintln!(
                        "{}",
                        "Warning: Using default database path. No configuration found.".yellow()
                    );
                    eprintln!("Default path: {}", configured_path);
                    eprintln!(
                        "Consider creating a configuration file at ~/.config/bkmr/config.toml"
                    );
                    eprintln!("or setting the BKMR_DB_URL environment variable.");

                    // Ask for confirmation when using default configuration
                    if !confirm("Continue with default database location?") {
                        eprintln!("Database creation cancelled.");
                        return Ok(());
                    }
                }

                configured_path.clone()
            }
        };

        // Check if the database file already exists
        if Path::new(&db_path).exists() {
            return Err(CliError::InvalidInput(format!(
                "Database already exists at: {}. Please choose a different path or delete the existing file.",
                db_path
            )));
        }

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&db_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    CliError::Io(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to create parent directories: {}", e),
                    ))
                })?;
            }
        }

        eprintln!("Creating new database at: {}", db_path);

        // Create the repository with the new path
        let repository = SqliteBookmarkRepository::from_url(&db_path)?;

        // Get a connection
        let mut conn = repository.get_connection()?;

        // Run migrations to set up the schema
        migration::init_db(&mut conn)?;

        // Clean the bookmark table to ensure we start with an empty database
        repository.empty_bookmark_table()?;

        eprintln!("Database created successfully at: {}", db_path);

        // Pre-fill the database with demo entries if requested
        if pre_fill {
            eprintln!("Pre-filling database with demo entries...");
            pre_fill_database(&repository, services)?;
            eprintln!("Demo entries added successfully!");
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn set_embeddable(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::SetEmbeddable {
        id,
        enable,
        disable,
    } = cli.command.unwrap()
    {
        let bookmark_service = services.bookmark_service.clone();

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
                eprintln!(
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

#[instrument(skip(cli), level = "debug")]
pub fn backfill(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Backfill { dry_run, force } = cli.command.unwrap() {
        // Check embedder type using services instead of global state
        if services.embedder.as_any().type_id() == std::any::TypeId::of::<DummyEmbedding>()
        {
            eprintln!("{}", "Error: Cannot backfill embeddings with DummyEmbedding active. Please use --openai flag.".red());
            return Err(CliError::CommandFailed(
                "DummyEmbedding active - embeddings not available".to_string(),
            ));
        }
        let bookmark_service = services.bookmark_service.clone();

        // Get bookmarks to process based on force flag
        let bookmarks = if force {
            bookmark_service.get_bookmarks_for_forced_backfill()?
        } else {
            bookmark_service.get_bookmarks_without_embeddings()?
        };

        if bookmarks.is_empty() {
            eprintln!("No embeddable bookmarks found that need embeddings (excluding bookmarks with '_imported_' tag)");
            return Ok(());
        }

        eprintln!(
            "Found {} bookmarks to process{}",
            bookmarks.len(),
            if force { " (force mode)" } else { "" }
        );

        if dry_run {
            // Just show the bookmarks that would be processed
            for bookmark in &bookmarks {
                eprintln!(
                    "Would update: {} (ID: {})",
                    bookmark.title,
                    bookmark.id.unwrap_or(0)
                );
            }
        } else {
            // Process each bookmark
            for bookmark in &bookmarks {
                if let Some(id) = bookmark.id {
                    eprintln!("Updating embedding for: {} (ID: {})", bookmark.title, id);
                    match bookmark_service.update_bookmark(bookmark.clone(), force) {
                        Ok(_) => eprintln!("  Successfully updated embedding"),
                        Err(e) => eprintln!("  Failed to update embedding: {}", e),
                    }
                }
            }
            eprintln!(
                "Completed embedding backfill for {} bookmarks",
                bookmarks.len()
            );
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn load_json(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::LoadJson { path, dry_run } = cli.command.unwrap() {
        eprintln!("Loading bookmarks from JSON array: {}", path);

        let bookmark_service = services.bookmark_service.clone();

        if dry_run {
            let count = bookmark_service.load_json_bookmarks(&path, true)?;
            eprintln!(
                "Dry run completed - would process {} bookmark entries",
                count
            );
            return Ok(());
        }

        // Process the bookmarks
        let processed_count = bookmark_service.load_json_bookmarks(&path, false)?;
        eprintln!(
            "Successfully processed {} bookmark entries",
            processed_count
        );
    }
    Ok(())
}

#[instrument(skip(cli), level = "debug")]
pub fn load_texts(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::LoadTexts {
        dry_run,
        force,
        path,
    } = cli.command.unwrap()
    {
        // Check embedder type using services instead of global state
        if services.embedder.as_any().type_id() == std::any::TypeId::of::<DummyEmbedding>()
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

        eprintln!("Loading text documents from NDJSON file: {}", path);
        eprintln!("(Expecting one JSON document per line)");

        let bookmark_service = services.bookmark_service.clone();

        if dry_run {
            let count = bookmark_service.load_texts(&path, true, force)?;
            eprintln!("Dry run completed - would process {} text entries", count);
            return Ok(());
        }

        // Process the texts
        let processed_count = bookmark_service.load_texts(&path, false, force)?;
        eprintln!("Successfully processed {} text entries", processed_count);
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn info(cli: Cli, services: &ServiceContainer, settings: &Settings) -> CliResult<()> {
    if let Commands::Info { show_schema } = cli.command.unwrap() {
        // Use settings parameter and services instead of global state
        let repository = SqliteBookmarkRepository::from_url(&settings.db_url)?;

        // Program version
        println!("Program Version: {}", env!("CARGO_PKG_VERSION"));

        // App configuration
        println!("\nConfiguration:");
        println!("  Database URL: {}", settings.db_url);
        println!("  FZF Height: {}", settings.fzf_opts.height);
        println!("  FZF Reverse: {}", settings.fzf_opts.reverse);
        println!("  FZF Show Tags: {}", settings.fzf_opts.show_tags);
        println!("  FZF Hide URL: {}", settings.fzf_opts.no_url);

        // Embedder type using services
        let embedder_type = if services.embedder.as_any().type_id()
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

        // Add system tag statistics
        println!("\nSystem Tags:");
        display_system_tag_stats(&repository)?;

        // Show schema if requested
        if show_schema {
            println!("\nDatabase Schema:");
            print_db_schema(&repository);
        }
    }
    Ok(())
}

fn display_system_tag_stats(repository: &SqliteBookmarkRepository) -> CliResult<()> {
    // Define known system tags
    let system_tags = [
        (
            "_snip_",
            "Snippet",
            "Code snippets that are copied to clipboard",
        ),
        ("_imported_", "Text", "Imported text documents"),
        ("_shell_", "Shell", "Shell scripts that are executed"),
        (
            "_md_",
            "Markdown",
            "Markdown documents that are rendered as HTML",
        ),
        (
            "_env_",
            "Environment",
            "Environment variables for shell sourcing",
        ),
    ];

    // Get all bookmarks
    let bookmarks = repository.get_all()?;

    // Count usage of each system tag
    for (tag_value, tag_name, description) in &system_tags {
        let count = bookmarks
            .iter()
            .filter(|b| b.tags.iter().any(|t| t.value() == *tag_value))
            .count();

        println!(
            "  {} ({}): {} entries - {}",
            tag_name.cyan(),
            tag_value.yellow(),
            count,
            description
        );
    }

    Ok(())
}

/// Pre-fills the database with a variety of demo entries to showcase bkmr's features
fn pre_fill_database(repository: &SqliteBookmarkRepository, services: &ServiceContainer) -> CliResult<()> {
    let embedder = &*services.embedder;

    // Create demo entries
    let demo_entries = vec![
        // Regular URLs
        (
            "https://github.com",
            "GitHub",
            "Platform for version control and collaboration",
            vec!["git", "development", "coding"],
        ),
        (
            "https://rust-lang.org",
            "Rust Programming Language",
            "A language empowering everyone to build reliable and efficient software",
            vec!["rust", "programming", "language"],
        ),
        (
            "https://crates.io",
            "Rust Package Registry",
            "The Rust community's crate registry",
            vec!["rust", "packages", "crates"],
        ),

        // Shell command URLs
        (
            "shell::echo 'Hello, World!'",
            "Hello World Shell Command",
            "Simple shell command that prints 'Hello, World!'",
            vec!["shell", "example", "hello_world"],
        ),
        (
            "shell::ls -la | grep '.rs$'",
            "List Rust Files",
            "Shell command to list all Rust files in the current directory",
            vec!["shell", "rust", "files", "list"],
        ),

        // URL with interpolation (date)
        (
            "https://example.com/report?date={{ current_date | strftime(\"%Y-%m-%d\") }}",
            "Daily Report",
            "Dynamic URL that includes today's date",
            vec!["report", "dynamic", "interpolation", "date"],
        ),
        (
            "https://api.example.com/data?from={{ current_date | subtract_days(7) | strftime(\"%Y-%m-%d\") }}&to={{ current_date | strftime(\"%Y-%m-%d\") }}",
            "Last 7 Days Data",
            "API URL for fetching last 7 days of data",
            vec!["api", "dynamic", "date_range", "interpolation"],
        ),

        // URL with environment variable interpolation
        (
            "https://api.service.com/v1/users?token={{ env('API_TOKEN', 'demo-token') }}",
            "API with Token",
            "Service API that uses an environment variable for authentication",
            vec!["api", "token", "environment", "interpolation"],
        ),

        // Code snippet with language tag
        (
            "println!(\"Hello, {}!\", \"Rust\");\n\nfn main() {\n    println!(\"This is a Rust snippet example\");\n}",
            "Rust Hello World Snippet",
            "Simple Rust code snippet demonstrating println",
            vec!["rust", "snippet", "code", "_snip_"],
        ),
        (
            "function greet(name) {\n    console.log(`Hello, ${name}!`);\n}\n\ngreet('JavaScript');",
            "JavaScript Greeting Function",
            "Simple JavaScript function that greets a person",
            vec!["javascript", "snippet", "function", "_snip_"],
        ),

        // Shell script snippet
        (
            "#!/bin/bash\n\necho \"Current directory:\"\npwd\n\necho \"\\nFiles:\"\nls -la",
            "Directory Info Script",
            "Shell script that shows current directory and lists files",
            vec!["bash", "script", "shell", "_snip_"],
        ),

        // Snippet with interpolation
        (
            "#!/bin/bash\n\n# Today's date: {{ current_date | strftime(\"%Y-%m-%d\") }}\n\necho \"Report for {{ current_date | strftime(\"%B %d, %Y\") }}\"",
            "Date Script with Interpolation",
            "Shell script with embedded date interpolation",
            vec!["bash", "date", "interpolation", "_snip_"],
        ),

        // SQL snippet
        (
            "SELECT *\nFROM users\nWHERE registration_date > '{{ current_date | subtract_days(30) | strftime(\"%Y-%m-%d\") }}'\nORDER BY username ASC;",
            "Recent Users SQL Query",
            "SQL query to find users registered in the last 30 days",
            vec!["sql", "query", "users", "_snip_"],
        ),

        // Markdown document
        (
            "# Meeting Notes: {{ current_date | strftime(\"%B %d, %Y\") }}\n\n## Agenda\n- Review last week's progress\n- Discuss current blockers\n- Plan for next sprint\n\n## Action Items\n- [ ] Document API changes\n- [ ] Complete code review\n- [ ] Deploy to staging",
            "Meeting Notes Template",
            "Template for taking meeting notes with dynamic date",
            vec!["markdown", "meeting", "template", "_imported_"],
        ),

        // URL with shell interpolation
        (
            "https://example.com/user/{{ \"whoami\" | shell }}",
            "User-specific Link",
            "URL that includes the current username via shell command",
            vec!["dynamic", "shell", "interpolation"],
        ),

        // Shell script (with _shell_ tag)
        (
            "#!/bin/bash\n\necho \"Running shell script bookmark...\"\necho \"Current directory: $(pwd)\"\nls -la",
            "Directory Info Shell Script",
            "Shell script that shows current directory and lists files",
            vec!["bash", "script", "shell", "_shell_"],
        ),

        // Shell script with interpolation
        (
            "#!/bin/bash\n\n# Today's date: {{ current_date | strftime(\"%Y-%m-%d\") }}\n\necho \"Report for {{ current_date | strftime(\"%B %d, %Y\") }}\"\necho \"Environment variables:\"\nenv | sort",
            "Environment Report Shell Script",
            "Shell script that reports environment variables and the current date",
            vec!["bash", "environment", "report", "_shell_"],
        ),

        // Markdown document
        (
            "# Markdown Example\n\n## Features\n\n- **Bold text**\n- *Italic text*\n- Lists\n- Links: [Example](https://example.com)\n\n## Code Snippets\n\n```rust\nfn hello() {\n    println!(\"Hello from Markdown!\");\n}\n```\n\n> This is a blockquote.\n\n![Image placeholder](https://via.placeholder.com/150)",
            "Markdown Demo Document",
            "Example of a markdown document with various formatting features",
            vec!["markdown", "example", "documentation", "_md_"],
        ),

        // Markdown with interpolation
        (
            "# Daily Report: {{ current_date | strftime(\"%B %d, %Y\") }}\n\n## Overview\n\nThis is an automatically generated report for {{ current_date | strftime(\"%Y-%m-%d\") }}.\n\n## Tasks\n\n- Review yesterday's progress\n- Plan today's work\n- Schedule meetings\n\n## Notes\n\n> Add your daily notes here.\n\n## Environment\n\n```\nUser: {{ \"whoami\" | shell }}\nHostname: {{ \"hostname\" | shell }}\n```",
            "Daily Report Template",
            "Markdown template for daily reports with date interpolation",
            vec!["markdown", "template", "report", "_md_"],
        ),

        // Environment variables
        (
            "# Environment variables for a dev project\nexport PROJECT_ROOT=\"$HOME/projects/myapp\"\nexport DATABASE_URL=\"postgres://localhost/myapp_dev\"\nexport API_KEY=\"dev_key_{{ current_date | strftime(\"%Y%m%d\") }}\"\n\n# Add the project bin to PATH\nexport PATH=\"$PROJECT_ROOT/bin:$PATH\"\n\necho \"Development environment loaded for MyApp\"",
            "Dev Environment",
            "Environment variables for the development project",
            vec!["postgres", "development", "env", "_env_"],
        ),
    ];

    // Add each entry to the database
    for (url, title, description, tags) in demo_entries {
        let mut tag_set = HashSet::new();
        for tag_str in tags {
            if let Ok(tag) = Tag::new(tag_str) {
                tag_set.insert(tag);
            }
        }

        match Bookmark::new(url, title, description, tag_set, embedder) {
            Ok(mut bookmark) => {
                // Set embeddable flag for regular URLs
                if url.starts_with("http") && !url.contains("{{") {
                    bookmark.set_embeddable(true);
                }

                // Add the bookmark to the repository
                if let Err(e) = repository.add(&mut bookmark) {
                    eprintln!("Failed to add demo bookmark {}: {}", title, e);
                }
            }
            Err(e) => {
                eprintln!("Failed to create demo bookmark {}: {}", title, e);
            }
        }
    }

    Ok(())
}

pub fn import_files(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    use crate::application::error::ApplicationError;
    use crate::config::{has_base_path, load_settings};
    use crate::exitcode;

    if let Some(Commands::ImportFiles {
        paths,
        update,
        delete_missing,
        dry_run,
        verbose,
        base_path,
    }) = cli.command
    {
        // Validate base path if provided
        if let Some(ref base_path_name) = base_path {
            let settings = load_settings(cli.config.as_deref())
                .map_err(|e| CliError::Other(format!("Failed to load configuration: {}", e)))?;

            if !has_base_path(&settings, base_path_name) {
                eprintln!(
                    "{}",
                    format!(
                        "Error: Base path '{}' not found in configuration",
                        base_path_name
                    )
                    .red()
                );
                eprintln!("Add it to your config.toml under [base_paths]:");
                eprintln!("  [base_paths]");
                eprintln!("  {} = \"$HOME/your/path\"", base_path_name);
                std::process::exit(exitcode::USAGE);
            }
        }

        let service = services.bookmark_service.clone();

        if dry_run {
            println!("{}", "Dry run mode - showing what would be done:".green());
        }

        match service.import_files(
            &paths,
            update,
            delete_missing,
            dry_run,
            verbose,
            base_path.as_deref(),
        ) {
            Ok((added, updated, deleted)) => {
                if dry_run {
                    println!(
                        "Would add: {}, update: {}, delete: {}",
                        added.to_string().green(),
                        updated.to_string().yellow(),
                        deleted.to_string().red()
                    );
                } else {
                    println!(
                        "Added: {}, Updated: {}, Deleted: {}",
                        added.to_string().green(),
                        updated.to_string().yellow(),
                        deleted.to_string().red()
                    );
                }
                Ok(())
            }
            Err(ApplicationError::DuplicateName {
                name,
                existing_id,
                file_path,
            }) => {
                eprintln!(
                    "{}",
                    format!("Error: Duplicate name '{}' found in {}", name, file_path).red()
                );
                eprintln!(
                    "Existing bookmark with same name already exists (ID: {})",
                    existing_id
                );
                eprintln!("Use --update flag to overwrite existing bookmarks with changed content");
                std::process::exit(exitcode::DUP);
            }
            Err(e) => {
                eprintln!("{}", format!("Import failed: {}", e).red());
                std::process::exit(exitcode::USAGE);
            }
        }
    } else {
        Err(CliError::InvalidInput(
            "Expected ImportFiles command".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::{init_test_env, setup_test_db, EnvGuard};

    #[test]
    fn given_valid_id_string_when_get_ids_then_returns_id_vector() {
        let ids = "1,2,3,4,5".to_string();
        let result = get_ids(ids);
        assert!(result.is_ok());
        let id_list = result.unwrap();
        assert_eq!(id_list, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn given_invalid_id_string_when_get_ids_then_returns_error() {
        let ids = "1,2,three,4,5".to_string();
        let result = get_ids(ids);
        assert!(result.is_err());
    }

    #[test]
    fn given_empty_database_when_pre_fill_then_creates_sample_bookmarks() {
        // Arrange
        let _ = init_test_env();
        let _guard = EnvGuard::new();
        let repository = setup_test_db();

        // Make sure we start with an empty database
        repository
            .empty_bookmark_table()
            .expect("Failed to empty bookmark table");

        // Verify database is initially empty
        let initial_bookmarks = repository.get_all().expect("Failed to get bookmarks");
        assert_eq!(
            initial_bookmarks.len(),
            0,
            "Database should be empty initially"
        );

        // Act - Create minimal service container for the test
        use crate::util::test_service_container::TestServiceContainer;
        let test_container = TestServiceContainer::new();
        let dummy_services = crate::infrastructure::di::service_container::ServiceContainer {
            bookmark_repository: test_container.bookmark_repository.clone(),
            embedder: test_container.embedder.clone(),
            bookmark_service: test_container.bookmark_service.clone(),
            tag_service: test_container.tag_service.clone(),
            action_service: test_container.action_service.clone(),
            clipboard_service: test_container.clipboard_service.clone(),
            template_service: test_container.template_service.clone(),
        };
        pre_fill_database(&repository, &dummy_services).expect("Failed to pre-fill database");

        // Assert
        let bookmarks = repository.get_all().expect("Failed to get bookmarks");

        // Verify we have added the expected number of demo entries
        assert!(
            !bookmarks.is_empty(),
            "Database should contain demo entries"
        );

        // Define expected entry types to check for
        let expected_types = vec![
            // Regular URLs
            ("https://github.com", false),
            ("https://rust-lang.org", false),
            // Shell command URLs
            ("shell::", false),
            // URL with interpolation (date)
            ("{{", false),
            // Code snippets
            ("_snip_", true),
            // Imported documents
            ("_imported_", true),
        ];

        // Check that each expected type exists in the database
        for (pattern, is_tag) in expected_types {
            let found = if is_tag {
                // Check for a tag containing the pattern
                bookmarks
                    .iter()
                    .any(|b| b.tags.iter().any(|t| t.value().contains(pattern)))
            } else {
                // Check for a URL containing the pattern
                bookmarks.iter().any(|b| b.url.contains(pattern))
            };

            assert!(
                found,
                "Database should contain an entry with {} '{}'",
                if is_tag { "tag" } else { "URL containing" },
                pattern
            );
        }

        // Check that we have entries with embeddable flag set
        let embeddable_entries = bookmarks.iter().filter(|b| b.embeddable).count();
        assert!(
            embeddable_entries > 0,
            "Database should contain entries with embeddable flag set"
        );

        // Test specific entries for correct data
        let github_entry = bookmarks.iter().find(|b| b.url == "https://github.com");
        assert!(github_entry.is_some(), "GitHub entry should exist");
        if let Some(entry) = github_entry {
            assert_eq!(entry.title, "GitHub");
            assert!(entry.tags.iter().any(|t| t.value() == "git"));
        }

        // Test that snippets are correctly marked
        let snippets = bookmarks
            .iter()
            .filter(|b| b.tags.iter().any(|t| t.value() == "_snip_"))
            .count();
        assert!(snippets > 0, "Database should contain snippet entries");
    }
}

// Temporary helper functions removed - using proper dependency injection
