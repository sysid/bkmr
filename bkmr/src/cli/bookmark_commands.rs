// src/cli/bookmark_commands.rs
use crate::application::actions::MarkdownAction;
use crate::config::Settings;
// Service container dependency injection implemented via SearchCommandHandler
use crate::application::services::action_service::ActionService;
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::tag_service::TagService;
use crate::application::services::template_service::TemplateService;
use crate::application::templates::bookmark_template::BookmarkTemplate;
use crate::cli::args::{Cli, Commands};
use crate::cli::backup_restore::{restore_from_backup, BackupRestoreRequest};
use crate::cli::display::lookup_embedded_ids;
use crate::cli::error::{CliError, CliResult};
use crate::cli::history_diff::{render_patch, MemoryState};
use crate::cli::process::{edit_bookmarks, execute_bookmark_default_action};
use crate::config::get_config_file_path;
use crate::domain::bookmark::Bookmark;
use crate::domain::error_context::CliErrorContext;
use crate::domain::history::{HistoryChange, HistoryEntry};
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::search::SemanticSearch;
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use crate::infrastructure::di::ServiceContainer;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::infrastructure::json::{write_bookmarks_as_json, JsonBookmarkView};
use crate::infrastructure::repositories::sqlite::repository::{
    print_db_schema, SqliteBookmarkRepository,
};
use crate::util::argument_processor::ArgumentProcessor;
use crate::util::helper::{confirm, ensure_int_vector, is_stdout_piped, should_use_color};
use crate::util::helper::{format_file_path, format_mtime};
use chrono;
use crossterm::style::{Color, Stylize};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::{fs, io};
use tracing::{info, instrument, warn};

// Helper function to get and validate IDs
fn get_ids(ids: String) -> CliResult<Vec<i32>> {
    // ensure_int_vector(&ids.split(',').map(String::from).collect())
    //     .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
    let string_vec: Vec<String> = ids.split(',').map(|s| s.trim().to_string()).collect();
    ensure_int_vector(&string_vec)
        .ok_or_else(|| CliError::InvalidIdFormat(format!("Invalid ID format: {}", ids)))
}

/// Format action description, incorporating custom opener if present
pub fn format_action_description(base_description: &str, opener: Option<&String>) -> String {
    match opener {
        Some(o) => format!("Open with {}", o),
        None => base_description.to_string(),
    }
}

#[instrument(skip(cli, services))]
pub fn semantic_search(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    let mut stderr = std::io::stderr();
    if let Commands::SemSearch {
        query,
        limit,
        non_interactive,
    } = cli.command.unwrap()
    {
        // Check if real embeddings are available (DummyEmbedding has dimensions == 0)
        if services.embedder.dimensions() == 0 {
            writeln!(
                stderr,
                "{}",
                "Error: Semantic search requires embeddings. Configure an embedding provider."
                    .red()
            )
            .cli_context("writing no-embedder error message to stderr")?;
            return Err(CliError::CommandFailed(
                "No embeddings available - configure an embedding provider".to_string(),
            ));
        }

        // Create the semantic search domain object
        let search = SemanticSearch::new(query, limit.map(|l| l as usize));

        // Perform semantic search
        let results = services
            .bookmark_service
            .semantic_search(&search)
            .cli_context("performing semantic search on bookmarks")?;

        if results.is_empty() {
            writeln!(stderr, "{}", "No bookmarks found".yellow())
                .cli_context("writing empty search result to stderr")?;
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
            io::stdout()
                .flush()
                .cli_context("flushing stdout after prompt")?;

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .cli_context("reading user input for bookmark IDs")?;

            let ids = get_ids(input.trim().to_string())
                .cli_context("parsing bookmark IDs from user input")?;
            for id in ids {
                if let Some(result) = results.iter().find(|r| r.bookmark.id == Some(id)) {
                    execute_bookmark_default_action(
                        &result.bookmark,
                        services.action_service.clone(),
                    )
                    .cli_context("executing default action for selected bookmark")?;
                } else {
                    writeln!(stderr, "Bookmark with ID {} not found in results", id)?;
                }
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli, bookmark_service, action_service, interpolation_service))]
pub fn open(
    cli: Cli,
    bookmark_service: Arc<dyn BookmarkService>,
    action_service: Arc<dyn ActionService>,
    interpolation_service: Arc<
        dyn crate::application::services::interpolation_service::InterpolationService,
    >,
) -> CliResult<()> {
    if let Commands::Open {
        ids,
        no_edit,
        file,
        script_args,
        stdout,
    } = cli.command.unwrap()
    {
        if file {
            // Handle direct file viewing
            handle_file_viewing(&ids).cli_context("handling direct file viewing")?;
        } else {
            // Handle bookmark opening (existing logic)

            for id in get_ids(ids).cli_context("parsing bookmark IDs for opening")? {
                if let Some(bookmark) = bookmark_service
                    .get_bookmark(id)
                    .cli_context("retrieving bookmark for opening")?
                {
                    if stdout {
                        action_service.record_access(&bookmark);
                        // Output interpolated content to stdout instead of executing
                        let content = interpolation_service
                            .render_bookmark_url(&bookmark)
                            .map_err(|e| {
                                CliError::CommandFailed(format!("Failed to render content: {}", e))
                            })?;
                        println!("{}", content);
                    } else {
                        // Use action service to execute default action
                        let base_description =
                            action_service.get_default_action_description(&bookmark);
                        let action_type =
                            format_action_description(base_description, bookmark.opener.as_ref());
                        eprintln!("Performing '{}' for: {}", action_type, bookmark.title);

                        // Execute default action with access recording handled by action service
                        action_service.execute_default_action_with_options(
                            &bookmark,
                            no_edit,
                            &script_args,
                        )?;
                    }
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
        opener: None,
        accessed_at: None,
        verified_at: None,
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
            | SystemTag::Memory
    ) {
        content.replace("\\n", "\n")
    } else {
        content.to_string()
    }
}

/// The explicit embedding request of the `--embed` and `--no-embed` flags (mutually exclusive).
fn embed_request(embed: bool, no_embed: bool) -> Option<bool> {
    match (embed, no_embed) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    }
}

#[instrument(skip(cli, bookmark_service, template_service))]
pub fn add(
    cli: Cli,
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
) -> CliResult<()> {
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
        open_with,
        embed,
        no_embed,
    } = cli.command.unwrap()
    {
        let requested = embed_request(embed, no_embed);

        // Convert bookmark_type string to SystemTag
        let system_tag = match bookmark_type.to_lowercase().as_str() {
            "snip" => SystemTag::Snippet,
            "text" => SystemTag::Text,
            "shell" => SystemTag::Shell,
            "md" | "markdown" => SystemTag::Markdown,
            "env" => SystemTag::Env,
            "mem" | "memory" => SystemTag::Memory,
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
                CliError::CommandFailed(format!("No bookmark found with ID {}", id))
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
                requested,
                open_with.as_deref(),
            )?;

            eprintln!(
                "Added bookmark: {} (ID: {})",
                bookmark.title,
                bookmark.id.unwrap_or(0)
            );
            return Ok(());
        }

        // Seed the value the template shows. A clone keeps the flag of its source.
        let source_flag = clone_id.map(|_| template.embeddable);
        template.embeddable =
            bookmark_service.resolve_embeddable(&template.tags, requested.or(source_flag));

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

                // CLI flag overrides whatever was set in the edit template;
                // otherwise fall back to the OPENER section parsed from the template
                let effective_opener = open_with.as_deref().or(edited_bookmark.opener.as_deref());

                // Add the edited bookmark
                match bookmark_service.add_bookmark(
                    &edited_bookmark.url,
                    Some(&edited_bookmark.title),
                    Some(&edited_bookmark.description),
                    Some(&edited_bookmark.tags),
                    false, // Don't fetch metadata since we've already edited it
                    // The saved template is the last explicit statement of intent
                    Some(edited_bookmark.embeddable),
                    effective_opener,
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

/// The IDs in their given order without repeats, so each is handled and reported once.
fn distinct_ids(ids: Vec<i32>) -> Vec<i32> {
    let mut seen = HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

fn report_failed_ids(action: &str, failed_ids: &[i32]) -> CliResult<()> {
    if failed_ids.is_empty() {
        return Ok(());
    }
    Err(CliError::CommandFailed(format!(
        "Failed to {} bookmarks with IDs: {:?}",
        action, failed_ids
    )))
}

#[instrument(skip(cli, bookmark_service))]
pub fn delete(cli: Cli, bookmark_service: Arc<dyn BookmarkService>) -> CliResult<()> {
    if let Commands::Delete { ids, force, yes } = cli.command.unwrap() {
        let id_list = distinct_ids(get_ids(ids)?);
        // One failed ID must not stop the others, so failures are reported at the end
        let mut failed_ids = Vec::new();
        let mut declined = false;

        for id in id_list {
            if force {
                match purge_with_confirmation(&bookmark_service, id) {
                    Ok(PurgeOutcome::Purged) => {}
                    Ok(PurgeOutcome::Declined) => declined = true,
                    Ok(PurgeOutcome::NotFound) => failed_ids.push(id),
                    Err(e) => {
                        eprintln!("Error purging bookmark with ID {}: {}", id, e);
                        failed_ids.push(id);
                    }
                }
                continue;
            }

            let bookmark = match bookmark_service.get_bookmark(id) {
                Ok(bookmark) => bookmark,
                Err(e) => {
                    eprintln!("Error reading bookmark with ID {}: {}", id, e);
                    failed_ids.push(id);
                    continue;
                }
            };
            if let Some(bookmark) = bookmark {
                eprintln!("Deleting: {} ({})", bookmark.title, bookmark.url);

                if yes || confirm("Confirm delete?") {
                    match bookmark_service.delete_bookmark(id) {
                        Ok(true) => eprintln!("Deleted bookmark with ID {}", id),
                        Ok(false) => {
                            eprintln!("Bookmark with ID {} not found", id);
                            failed_ids.push(id);
                        }
                        Err(e) => {
                            eprintln!("Error deleting bookmark with ID {}: {}", id, e);
                            failed_ids.push(id);
                        }
                    }
                } else {
                    eprintln!("Deletion cancelled");
                    declined = true;
                }
            } else {
                eprintln!("Bookmark with ID {} not found", id);
                failed_ids.push(id);
            }
        }

        report_failed_ids("delete", &failed_ids)?;
        // A script or an agent without stdin must not read a declined prompt as the bookmark being
        // gone. Failed IDs go first: they need the caller's attention whatever else was declined.
        if declined {
            return Err(CliError::OperationAborted);
        }
    }
    Ok(())
}

enum PurgeOutcome {
    Purged,
    Declined,
    NotFound,
}

fn purge_with_confirmation(
    bookmark_service: &Arc<dyn BookmarkService>,
    id: i32,
) -> CliResult<PurgeOutcome> {
    let bookmark = bookmark_service.get_bookmark(id)?;
    let history_count = bookmark_service.get_history(Some(id), i64::MAX)?.len();

    match &bookmark {
        // Not the content: this command exists to remove secrets, and terminal scrollback and agent
        // transcripts are out of reach of any scrub.
        Some(bookmark) => eprintln!(
            "Permanently removing: {} (ID {}) and {} history entries",
            bookmark.title, id, history_count
        ),
        None if history_count > 0 => eprintln!(
            "Permanently removing {} history entries of deleted bookmark {}",
            history_count, id
        ),
        // Nothing left that a prompt could protect. The text of a bookmark deleted earlier, an _env_
        // secret for example, or of a purge that failed halfway can still be in the files.
        None => {
            bookmark_service.purge_bookmark(id)?;
            eprintln!(
                "Bookmark with ID {} not found. Removed traces of deleted text from the database files",
                id
            );
            return Ok(PurgeOutcome::NotFound);
        }
    }

    if confirm("Confirm permanent removal? This cannot be restored") {
        bookmark_service.purge_bookmark(id)?;
        eprintln!("Purged bookmark with ID {}", id);
        Ok(PurgeOutcome::Purged)
    } else {
        eprintln!("Purge cancelled");
        Ok(PurgeOutcome::Declined)
    }
}

#[instrument(skip(cli, bookmark_service, settings))]
pub fn history(
    cli: Cli,
    bookmark_service: Arc<dyn BookmarkService>,
    settings: &Settings,
) -> CliResult<()> {
    if let Commands::History {
        id,
        limit,
        is_json,
        patch,
    } = cli.command.unwrap()
    {
        if limit <= 0 {
            return Err(CliError::InvalidInput(
                "Limit must be a positive integer".to_string(),
            ));
        }
        let entries = bookmark_service.get_history(id, limit)?;

        if is_json {
            let json: Vec<serde_json::Value> = entries
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "history_id": entry.history_id,
                        "bookmark_id": entry.bookmark_id,
                        "change": entry.change.as_str(),
                        "url": entry.url,
                        "title": entry.title,
                        "description": entry.description,
                        "tags": entry.tags,
                        "changed_at": entry.changed_at.to_rfc3339(),
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| "[]".to_string())
            );
        } else if entries.is_empty() {
            eprintln!("No history found");
        } else if let (true, Some(id)) = (patch, id) {
            let use_color = diff_color(settings);
            // Each entry holds the state before its change, so the state after it is the next newer
            // entry, or for the newest one the bookmark as it is now.
            let mut newer = bookmark_service
                .get_bookmark(id)?
                .map(|b| MemoryState::from_bookmark(&b));
            let mut newer_label = current_label(id);
            for entry in &entries {
                println!("{}", history_line(entry));
                let older = MemoryState::from_entry(entry);
                let older_label = history_label(entry);
                match (&entry.change, &newer) {
                    (HistoryChange::Delete, _) => print!(
                        "{}",
                        render_patch(Some(&older), None, &older_label, "/dev/null", use_color)
                    ),
                    (HistoryChange::Update, Some(after)) => print!(
                        "{}",
                        render_patch(
                            Some(&older),
                            Some(after),
                            &older_label,
                            &newer_label,
                            use_color
                        )
                    ),
                    // Deleted after it stopped being a memory, which history does not record, so
                    // the state this update led to is gone
                    (HistoryChange::Update, None) => println!(
                        "(no diff: #{} no longer exists, and its state after this update was not kept)",
                        id
                    ),
                }
                println!();
                newer = Some(older);
                newer_label = older_label;
            }
        } else {
            for entry in &entries {
                println!("{}", history_line(entry));
            }
        }
    }
    Ok(())
}

/// One row of the `history` listing
fn history_line(entry: &HistoryEntry) -> String {
    format!(
        "{}\t#{}\t{}\t{} UTC\t{}",
        entry.history_id,
        entry.bookmark_id,
        entry.change.as_str(),
        entry.changed_at.format("%Y-%m-%d %H:%M"),
        entry.title
    )
}

/// How the bookmark as it is now is named in a diff header
fn current_label(id: i32) -> String {
    format!("#{} current", id)
}

/// Diffs are colored on a terminal unless the user turned color off
fn diff_color(settings: &Settings) -> bool {
    should_use_color(settings.no_color, !is_stdout_piped())
}

/// How a saved state is named in a diff header
fn history_label(entry: &HistoryEntry) -> String {
    format!(
        "history {} ({} UTC)",
        entry.history_id,
        entry.changed_at.format("%Y-%m-%d %H:%M")
    )
}

#[instrument(skip(cli, bookmark_service, settings))]
pub fn restore(
    cli: Cli,
    bookmark_service: Arc<dyn BookmarkService>,
    settings: &Settings,
) -> CliResult<()> {
    if let Commands::Restore {
        history_id,
        from,
        ids,
        yes,
        dry_run,
        is_json,
    } = cli.command.unwrap()
    {
        if let Some(path) = from {
            let request = BackupRestoreRequest {
                path: &path,
                ids: &ids,
                yes,
                dry_run,
                is_json,
                color: diff_color(settings),
            };
            return restore_from_backup(&request, bookmark_service);
        }
        // clap requires one of the two
        let history_id = history_id.ok_or_else(|| {
            CliError::InvalidInput("a history ID or --from is required".to_string())
        })?;
        if dry_run {
            let preview = bookmark_service.preview_restore(history_id)?;
            let old_label = match &preview.current {
                Some(_) => current_label(preview.entry.bookmark_id),
                None => "/dev/null".to_string(),
            };
            // The bookmark the restore would write, not the entry, so the diff shows what it does
            let patch = render_patch(
                preview
                    .current
                    .as_ref()
                    .map(MemoryState::from_bookmark)
                    .as_ref(),
                Some(&MemoryState::from_bookmark(&preview.restored)),
                &old_label,
                &history_label(&preview.entry),
                diff_color(settings),
            );
            if patch.is_empty() {
                eprintln!(
                    "Restoring history entry {} would change nothing",
                    history_id
                );
            } else {
                print!("{}", patch);
            }
            return Ok(());
        }
        let bookmark = bookmark_service.restore_bookmark(history_id)?;
        eprintln!(
            "Restored: {} (ID: {})",
            bookmark.title,
            bookmark.id.unwrap_or(0)
        );
    }
    Ok(())
}

#[instrument(skip(cli, bookmark_service))]
pub fn verify(cli: Cli, bookmark_service: Arc<dyn BookmarkService>) -> CliResult<()> {
    if let Commands::Verify { ids } = cli.command.unwrap() {
        // One failed ID must not stop the others, so failures are reported at the end
        let mut failed_ids = Vec::new();
        for id in distinct_ids(ids) {
            match bookmark_service.verify_bookmark(id) {
                Ok(()) => eprintln!("Verified bookmark with ID {}", id),
                Err(e) => {
                    eprintln!("Error verifying bookmark with ID {}: {}", id, e);
                    failed_ids.push(id);
                }
            }
        }
        report_failed_ids("verify", &failed_ids)?;
    }
    Ok(())
}

#[instrument(skip(cli, bookmark_service, tag_service))]
pub fn update(
    cli: Cli,
    bookmark_service: Arc<dyn BookmarkService>,
    tag_service: Arc<dyn TagService>,
) -> CliResult<()> {
    if let Commands::Update {
        ids,
        tags,
        tags_not,
        force,
        title,
        description,
        url,
        open_with,
        embed,
        no_embed,
    } = cli.command.unwrap()
    {
        let id_list = get_ids(ids)?;

        if embed && no_embed {
            return Err(CliError::InvalidInput(
                "Cannot specify both --embed and --no-embed".to_string(),
            ));
        }

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                eprintln!("Updating: {} ({})", bookmark.title, bookmark.url);

                // Handle embedding flag changes
                if embed || no_embed {
                    let updated = bookmark_service.set_bookmark_embeddable(id, embed)?;
                    // The policy can override the request, so report the outcome
                    eprintln!(
                        "Embedding {}: {}",
                        if updated.embeddable {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        updated.title
                    );
                }

                let has_changes = tags.is_some()
                    || tags_not.is_some()
                    || title.is_some()
                    || description.is_some()
                    || url.is_some()
                    || open_with.is_some();
                if !has_changes {
                    continue;
                }

                // All changes go into one write, so that one command is one undo step in history.
                // Fetch latest version to avoid stale data after the embedding change.
                let Some(mut latest_bookmark) = bookmark_service.get_bookmark(id)? else {
                    continue;
                };

                if force && tags.is_some() {
                    // Replace all tags
                    let tag_str = tags.as_ref().unwrap();
                    let parsed_tags = tag_service.parse_tag_string(tag_str)?;
                    let tag_set = parsed_tags.into_iter().collect::<HashSet<_>>();

                    latest_bookmark.set_tags(tag_set)?;
                    eprintln!("Tags replaced: {}", latest_bookmark.formatted_tags());
                } else {
                    // Add tags if provided
                    if let Some(tag_str) = &tags {
                        for tag in tag_service.parse_tag_string(tag_str)? {
                            latest_bookmark.add_tag(tag)?;
                        }
                        eprintln!("Tags added: {}", latest_bookmark.formatted_tags());
                    }

                    // Remove tags if provided
                    if let Some(tag_str) = &tags_not {
                        for tag in tag_service.parse_tag_string(tag_str)? {
                            let _ = latest_bookmark.remove_tag(&tag);
                        }
                        eprintln!("Tags removed: {}", latest_bookmark.formatted_tags());
                    }
                }

                // Update scalar fields if provided (title, description, url, opener)
                if let Some(ref t) = title {
                    latest_bookmark.title = t.clone();
                    eprintln!("Title set to: {}", t);
                }
                if let Some(ref d) = description {
                    latest_bookmark.description = d.clone();
                    eprintln!("Description set to: {}", d);
                }
                if let Some(ref u) = url {
                    latest_bookmark.url = u.clone();
                    eprintln!("URL set to: {}", u);
                }
                if let Some(ref opener) = open_with {
                    latest_bookmark.opener = if opener.is_empty() {
                        None
                    } else {
                        Some(opener.clone())
                    };
                    if opener.is_empty() {
                        eprintln!("Custom opener cleared");
                    } else {
                        eprintln!("Custom opener set to: {}", opener);
                    }
                }
                bookmark_service.update_bookmark(latest_bookmark, false)?;
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }
    }
    Ok(())
}

#[instrument(skip(cli, bookmark_service, template_service, settings))]
pub fn edit(
    cli: Cli,
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    if let Commands::Edit { ids, force_db } = cli.command.unwrap() {
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

        edit_bookmarks(
            id_list,
            force_db,
            bookmark_service,
            template_service,
            settings,
        )?;
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn show(cli: Cli, services: &ServiceContainer, settings: &Settings) -> CliResult<()> {
    if let Commands::Show { ids, is_json } = cli.command.unwrap() {
        let bookmark_service = services.bookmark_service.clone();

        let id_list = get_ids(ids)?;
        let mut bookmarks = Vec::new();

        for id in id_list {
            if let Some(bookmark) = bookmark_service.get_bookmark(id)? {
                // Report the state before this access: otherwise "last accessed" always reads "now"
                services.action_service.record_access(&bookmark);
                bookmarks.push(bookmark);
            } else {
                eprintln!("Bookmark with ID {} not found", id);
            }
        }

        let embedded_ids = lookup_embedded_ids(&*bookmark_service);

        if is_json {
            // Output bookmarks as JSON
            let json_views =
                JsonBookmarkView::from_domain_collection(&bookmarks, embedded_ids.as_ref());
            write_bookmarks_as_json(&json_views)?;
        } else {
            let use_color = should_use_color(settings.no_color, !is_stdout_piped());

            // Output bookmarks in detailed format (existing behavior)
            for bookmark in &bookmarks {
                print!(
                    "{}",
                    show_bookmark_details(bookmark, embedded_ids.as_ref(), use_color, services)
                );
            }
        }
    }
    Ok(())
}

/// Shows detailed information about a bookmark, can be used by both show command and FZF CTRL-P
#[instrument(level = "debug")]
pub fn show_bookmark_details(
    bookmark: &Bookmark,
    embedded_ids: Option<&HashSet<i32>>,
    use_color: bool,
    services: &ServiceContainer,
) -> String {
    // Get the action service
    let action_service = services.action_service.clone();

    // Get the action description, incorporating custom opener if present
    let base_description = action_service.get_default_action_description(bookmark);
    let action_description = format_action_description(base_description, bookmark.opener.as_ref());

    format_bookmark_details(
        bookmark,
        &action_description,
        bookmark.has_embedding_in(embedded_ids),
        use_color,
    )
}

/// `has_embedding` is `None` when the lookup failed.
fn format_bookmark_details(
    bookmark: &Bookmark,
    action_description: &str,
    has_embedding: Option<bool>,
    use_color: bool,
) -> String {
    // Styling must be skipped, not just disabled: crossterm writes reset codes even with
    // colors turned off, which would end up in piped output.
    let paint = |text: String, color: Color| {
        if use_color {
            text.with(color).to_string()
        } else {
            text
        }
    };

    // Format all the bookmark details
    let mut details = format!(
        "{} {} [{}] ({})\n  URL/Content: {}\n{}\n  Opener: {}\n  Access count: {}\n  Last accessed: {}\n  Verified: {}\n  Created: {}\n  Updated: {}\n  Embeddable: {}\n  Has embedding: {}\n  Content hash: {}\n  Default Action: {}",
        paint(
            bookmark.id.map_or("?".to_string(), |id| id.to_string()),
            Color::Blue
        ),
        paint(bookmark.title.clone(), Color::Green),
        paint(bookmark.formatted_tags(), Color::Yellow),
        paint(action_description.to_string(), Color::Cyan),
        bookmark.url,
        // Colored as a whole: URL/Content can span many lines, so the label alone is easy to miss
        paint(
            format!("  Description: {}", bookmark.description),
            Color::Magenta
        ),
        bookmark.opener.as_deref().unwrap_or("N/A"),
        bookmark.access_count,
        bookmark.accessed_at.map_or("never".to_string(), |dt| dt.to_string()),
        bookmark.verified_at.map_or("never".to_string(), |dt| dt.to_string()),
        bookmark.created_at.map_or("N/A".to_string(), |dt| dt.to_string()),
        bookmark.updated_at,
        bookmark.embeddable,
        has_embedding.map_or("unknown".to_string(), |has| has.to_string()),
        bookmark.content_hash_hex().unwrap_or_else(|| "N/A".to_string()),
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
    if let Some(file_hash) = &bookmark.file_hash {
        details.push_str(&format!("\n  File hash: {}", file_hash));
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
            // Get the action description, incorporating custom opener if present
            let base_description = action_service.get_default_action_description(bookmark);
            let action_description =
                format_action_description(base_description, bookmark.opener.as_ref());

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

/// Clear all embeddings from vec_bookmarks and reset all content hashes.
/// Shared by `clear-embeddings` and `backfill --force`.
fn purge_all_embeddings(services: &ServiceContainer) -> CliResult<()> {
    services.vector_repository.clear_all()?;
    services.bookmark_repository.clear_all_content_hashes()?;
    Ok(())
}

pub fn clear_embeddings(_cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    let has = services.vector_repository.has_embeddings()?;
    if !has {
        eprintln!("No embeddings found — nothing to clear.");
        return Ok(());
    }

    eprint!("This will delete ALL embeddings and content hashes. Continue? [y/N] ");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| CliError::CommandFailed(format!("Failed to read input: {}", e)))?;
    if !input.trim().eq_ignore_ascii_case("y") {
        return Err(CliError::OperationAborted);
    }

    purge_all_embeddings(services)?;
    info!("All embeddings and content hashes cleared");
    eprintln!("Cleared all embeddings and content hashes.");
    Ok(())
}

#[instrument(skip(cli, services), level = "debug")]
pub fn backfill(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Backfill {
        dry_run,
        force,
        prune,
    } = cli.command.unwrap()
    {
        // Check if real embeddings are available
        if services.embedder.dimensions() == 0 {
            eprintln!(
                "{}",
                "Error: Cannot backfill embeddings without an embedding provider configured.".red()
            );
            return Err(CliError::CommandFailed(
                "No embedding provider configured - embeddings not available".to_string(),
            ));
        }
        let bookmark_service = services.bookmark_service.clone();

        if prune {
            let pruned = bookmark_service.prune_embeddings(dry_run)?;
            let action = if dry_run { "Would remove" } else { "Removed" };
            for id in &pruned {
                eprintln!("{} vector (ID: {})", action, id);
            }
            eprintln!("{} {} stale vectors", action, pruned.len());
            return Ok(());
        }

        // For force mode, clear all existing embeddings and content hashes first.
        // A dry run must not touch them.
        if force && !dry_run {
            eprintln!("Force mode: clearing all existing embeddings and content hashes...");
            purge_all_embeddings(services)?;
        }

        // Get bookmarks to process based on force flag
        let bookmarks = if force {
            bookmark_service.get_bookmarks_for_forced_backfill()?
        } else {
            bookmark_service.get_bookmarks_without_embeddings()?
        };

        if bookmarks.is_empty() {
            eprintln!("No embeddable bookmarks found that need embeddings");
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
            // One failed bookmark must not stop the others, so failures are reported at the end
            let mut failed_ids = Vec::new();
            for bookmark in &bookmarks {
                if let Some(id) = bookmark.id {
                    eprintln!("Updating embedding for: {} (ID: {})", bookmark.title, id);
                    match bookmark_service.embed_bookmark(id) {
                        Ok(true) => eprintln!("  Successfully updated embedding"),
                        Ok(false) => eprintln!("  No embedding produced for this content"),
                        Err(e) => {
                            eprintln!("  Failed to update embedding: {}", e);
                            failed_ids.push(id);
                        }
                    }
                }
            }
            info!(count = bookmarks.len(), force = force, "Backfill complete");
            eprintln!(
                "Completed embedding backfill for {} bookmarks",
                bookmarks.len()
            );
            report_failed_ids("embed", &failed_ids)?;
        }
    }
    Ok(())
}

#[instrument(skip(cli))]
pub fn load_json(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::LoadJson {
        path,
        dry_run,
        embed,
        no_embed,
    } = cli.command.unwrap()
    {
        eprintln!("Loading bookmarks from JSON array: {}", path);
        let requested = embed_request(embed, no_embed);

        let bookmark_service = services.bookmark_service.clone();

        if dry_run {
            let count = bookmark_service.load_json_bookmarks(&path, true, requested)?;
            eprintln!(
                "Dry run completed - would process {} bookmark entries",
                count
            );
            return Ok(());
        }

        // Process the bookmarks
        let processed_count = bookmark_service.load_json_bookmarks(&path, false, requested)?;
        info!(count = processed_count, path = %path, "JSON load complete");
        eprintln!(
            "Successfully processed {} bookmark entries",
            processed_count
        );
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

        // Configuration section
        println!("\nConfiguration:");

        // Config file status
        if let Some(config_path) = get_config_file_path() {
            let exists = config_path.exists();
            println!(
                "  Config file: {} ({})",
                config_path.display(),
                if exists { "exists" } else { "not found" }
            );
        } else {
            println!("  Config file: (unable to determine path)");
        }

        // Config source
        println!("  Source: {:?}", settings.config_source);

        // Database URL with file size
        let db_info = if let Ok(metadata) = fs::metadata(&settings.db_url) {
            format!("{} ({})", settings.db_url, format_file_size(metadata.len()))
        } else {
            format!("{} (file not found)", settings.db_url)
        };
        println!("  Database: {}", db_info);

        // FZF options - all of them
        println!("  FZF Height: {}", settings.fzf_opts.height);
        println!("  FZF Reverse: {}", settings.fzf_opts.reverse);
        println!("  FZF Show Tags: {}", settings.fzf_opts.show_tags);
        println!("  FZF Hide URL: {}", settings.fzf_opts.no_url);
        println!("  FZF Show Action: {}", settings.fzf_opts.show_action);
        println!("  FZF Show File Info: {}", settings.fzf_opts.show_file_info);

        // Shell options
        println!("  Shell Interactive: {}", settings.shell_opts.interactive);

        // Embedder type using dimensions check
        let embedder_type = if services.embedder.dimensions() == 0 {
            "DummyEmbedding (embeddings disabled)".to_string()
        } else {
            format!(
                "Embeddings enabled (dimensions: {})",
                services.embedder.dimensions()
            )
        };
        println!("  Embedder: {}", embedder_type);

        // Base paths section
        println!("\n  Base Paths:");
        if settings.base_paths.is_empty() {
            println!("    (none configured)");
        } else {
            for (name, value) in &settings.base_paths {
                let expanded = shellexpand::full(value)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|_| value.clone());
                if expanded != *value {
                    println!("    {}: {} -> {}", name, value, expanded);
                } else {
                    println!("    {}: {}", name, value);
                }
            }
        }

        // Environment overrides section
        println!("\n  Environment Overrides:");
        display_env_var("BKMR_DB_URL", false);
        display_env_var("BKMR_FZF_OPTS", false);
        display_env_var("BKMR_SHELL_INTERACTIVE", false);
        display_env_var("FASTEMBED_CACHE_DIR", false);

        // Database statistics
        let bookmarks = repository.get_all()?;
        let bookmark_count = bookmarks.len();
        let file_imported_count = bookmarks.iter().filter(|b| b.file_path.is_some()).count();

        println!("\nDatabase Statistics:");
        println!("  Total Bookmarks: {}", bookmark_count);
        println!("  File-imported: {}", file_imported_count);

        // Get tag statistics
        let tags = repository.get_all_tags()?;
        println!("  Total Tags: {}", tags.len());
        println!("  Top 5 Tags:");
        for (tag, count) in tags.iter().take(5) {
            println!("    {} ({})", tag.value(), count);
        }

        // Embedding statistics — display only: a vector store failure shows as
        // "no embeddings" here rather than hiding the rest of the info output.
        let embeddable_count = bookmarks.iter().filter(|b| b.embeddable).count();
        let vec_has_embeddings = services.vector_repository.has_embeddings().unwrap_or(false);
        let vec_dims = services.vector_repository.get_dimensions().unwrap_or(None);
        let vec_embedded_count = services
            .vector_repository
            .get_embedded_ids()
            .map(|ids| ids.len())
            .unwrap_or(0);

        println!("\nEmbeddings:");
        println!(
            "  Model: {} ({} dims)",
            settings.embeddings.model,
            services.embedder.dimensions()
        );
        let cache_dir = crate::infrastructure::embeddings::FastEmbedEmbedding::cache_dir();
        let cache_path = std::path::Path::new(&cache_dir);
        let model_downloaded = cache_path.exists()
            && cache_path
                .read_dir()
                .map_or(false, |mut d| d.next().is_some());
        println!(
            "  Model cache: {} ({})",
            cache_dir,
            if model_downloaded {
                "downloaded"
            } else {
                "not downloaded"
            }
        );
        println!(
            "  Embedded: {} of {} embeddable bookmarks",
            vec_embedded_count, embeddable_count
        );
        let policy: Vec<String> = settings
            .embeddings
            .policy
            .entries()
            .iter()
            .map(|(key, mode)| format!("{}={}", key, mode))
            .collect();
        println!("  Policy: {}", policy.join(", "));
        if let Some(dims) = vec_dims {
            println!(
                "  Vector table: vec_bookmarks ({} dims, {} rows)",
                dims, vec_embedded_count
            );
        } else if !vec_has_embeddings {
            println!("  Vector table: vec_bookmarks (empty)");
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

/// Format file size in human-readable form
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Display environment variable status
fn display_env_var(name: &str, mask_value: bool) {
    match std::env::var(name) {
        Ok(value) => {
            if mask_value {
                println!("    {}: configured", name);
            } else {
                println!("    {}: {}", name, value);
            }
        }
        Err(_) => {
            println!("    {}: not set", name);
        }
    }
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
pub fn pre_fill_database(repository: &SqliteBookmarkRepository) -> CliResult<()> {
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

        match Bookmark::new(url, title, description, tag_set) {
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

pub fn import_files(cli: Cli, services: &ServiceContainer, settings: &Settings) -> CliResult<()> {
    use crate::application::error::ApplicationError;
    use crate::config::has_base_path;
    use crate::exitcode;

    if let Some(Commands::ImportFiles {
        paths,
        update,
        delete_missing,
        dry_run,
        verbose,
        base_path,
        embed,
        no_embed,
    }) = cli.command
    {
        // Validate base path if provided
        if let Some(ref base_path_name) = base_path {
            if !has_base_path(settings, base_path_name) {
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
            embed_request(embed, no_embed),
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
                std::process::exit(exitcode::FAILURE);
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
    fn given_stored_bookmark_when_format_details_then_shows_all_metadata() {
        use chrono::{TimeZone, Utc};
        let mut tags = HashSet::new();
        tags.insert(Tag::new("alpha").unwrap());
        let mut bookmark =
            Bookmark::new("https://example.com", "Example", "Some text", tags).unwrap();
        bookmark.id = Some(42);
        bookmark.access_count = 7;
        bookmark.created_at = Some(Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap());
        bookmark.updated_at = Utc.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap();
        bookmark.accessed_at = Some(Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap());
        bookmark.content_hash = Some(vec![0xab, 0x01]);
        bookmark.embeddable = false;
        bookmark.opener = Some("vim".to_string());
        bookmark.file_path = Some("/notes/a.md".to_string());
        bookmark.file_mtime = Some(0);
        bookmark.file_hash = Some("cafe".to_string());

        let details = format_bookmark_details(&bookmark, "Open with vim", Some(true), false);

        assert!(details.contains("URL/Content: https://example.com"));
        assert!(details.contains("Access count: 7"));
        assert!(details.contains("Last accessed: 2026-03-04 05:06:07 UTC"));
        assert!(details.contains("Created: 2026-01-02 03:04:05 UTC"));
        assert!(details.contains("Updated: 2026-02-03 04:05:06 UTC"));
        assert!(details.contains("Opener: vim"));
        assert!(details.contains("Embeddable: false"));
        assert!(details.contains("Has embedding: true"));
        assert!(details.contains("Content hash: ab01"));
        assert!(details.contains("Default Action: Open with vim"));
        assert!(details.contains("Source File: /notes/a.md"));
        assert!(details.contains("File hash: cafe"));
    }

    #[test]
    fn given_sparse_bookmark_when_format_details_then_marks_missing_values() {
        let mut bookmark =
            Bookmark::new("https://example.com", "Example", "", HashSet::new()).unwrap();
        bookmark.created_at = None;

        let details = format_bookmark_details(&bookmark, "Open in browser", Some(false), false);

        assert!(details.contains("Last accessed: never"));
        assert!(details.contains("Created: N/A"));
        assert!(details.contains("Opener: N/A"));
        assert!(details.contains("Content hash: N/A"));
        assert!(details.contains("Has embedding: false"));
        assert!(!details.contains("Source File"));
        assert!(!details.contains("File hash"));
    }

    #[test]
    fn given_failed_embedding_lookup_when_format_details_then_says_unknown() {
        // A failed lookup must not read as "this bookmark has no embedding"
        let bookmark = Bookmark::new("https://example.com", "Example", "", HashSet::new()).unwrap();

        let details = format_bookmark_details(&bookmark, "Open in browser", None, false);

        assert!(details.contains("Has embedding: unknown"));
    }

    #[test]
    fn given_description_when_format_details_then_colors_it_unlike_the_url() {
        let bookmark = Bookmark::new(
            "https://example.com",
            "Example",
            "Some text",
            HashSet::new(),
        )
        .unwrap();

        let details = format_bookmark_details(&bookmark, "Open in browser", Some(false), true);

        let colored_description = "  Description: Some text".magenta().to_string();
        assert!(details.contains(&colored_description));
        assert!(details.contains("\n  URL/Content: https://example.com\n"));
    }

    #[test]
    fn given_color_off_when_format_details_then_writes_no_escape_codes() {
        let bookmark = Bookmark::new(
            "https://example.com",
            "Example",
            "Some text",
            HashSet::new(),
        )
        .unwrap();

        let details = format_bookmark_details(&bookmark, "Open in browser", Some(false), false);

        assert!(!details.contains('\u{1b}'));
        assert!(details.contains("\n  Description: Some text\n"));
    }

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

        // Verify database is initially empty
        let initial_bookmarks = repository.get_all().expect("Failed to get bookmarks");
        assert_eq!(
            initial_bookmarks.len(),
            0,
            "Database should be empty initially"
        );

        // Act - Use DummyEmbedding directly
        pre_fill_database(&repository).expect("Failed to pre-fill database");

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
