// src/cli/hsearch_handler.rs
use std::io::Write;

use crossterm::style::Stylize;
use termcolor::StandardStream;

use crate::cli::args::{Cli, Commands};
use crate::cli::error::{CliError, CliResult};
use crate::domain::error_context::CliErrorContext;
use crate::domain::search::{HybridSearch, SearchMode};
use crate::domain::tag::Tag;
use crate::infrastructure::di::ServiceContainer;
use crate::util::helper::is_stdout_piped;

/// Parse a comma-separated tag string into a HashSet<Tag>
fn parse_tags(s: &str) -> Option<std::collections::HashSet<Tag>> {
    let tags: std::collections::HashSet<Tag> = s
        .split(',')
        .filter_map(|t| {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                None
            } else {
                Tag::new(trimmed).ok()
            }
        })
        .collect();
    if tags.is_empty() { None } else { Some(tags) }
}

pub fn hybrid_search(
    mut stderr: StandardStream,
    cli: Cli,
    services: &ServiceContainer,
) -> CliResult<()> {
    if let Commands::HSearch {
        query,
        tags_all,
        tags_all_not,
        tags_any,
        tags_any_not,
        tags_exact,
        mode,
        limit,
        is_json,
        is_fuzzy: _is_fuzzy,
        stdout: _stdout,
        non_interactive,
    } = cli.command.unwrap()
    {
        // Build the hybrid search query
        let mut search = HybridSearch::new(query);
        search.limit = limit.map(|l| l as usize);
        search.mode = if mode == "exact" {
            SearchMode::Exact
        } else {
            SearchMode::Hybrid
        };

        // Apply tag filters
        search.tags_all = tags_all.as_deref().and_then(parse_tags);
        search.tags_all_not = tags_all_not.as_deref().and_then(parse_tags);
        search.tags_any = tags_any.as_deref().and_then(parse_tags);
        search.tags_any_not = tags_any_not.as_deref().and_then(parse_tags);
        search.tags_exact = tags_exact.as_deref().and_then(parse_tags);

        // Perform hybrid search
        let results = services
            .bookmark_service
            .hybrid_search(&search)
            .cli_context("performing hybrid search on bookmarks")?;

        if results.is_empty() {
            writeln!(stderr, "{}", "No bookmarks found".yellow())
                .cli_context("writing empty search result to stderr")?;
            return Ok(());
        }

        let is_piped = is_stdout_piped();

        if is_json {
            // JSON output with rrf_score
            let json_results: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.bookmark.id.unwrap_or(0),
                        "url": r.bookmark.url,
                        "title": r.bookmark.title,
                        "description": r.bookmark.description,
                        "tags": r.bookmark.formatted_tags(),
                        "rrf_score": r.rrf_score,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json_results)
                    .unwrap_or_else(|_| "[]".to_string())
            );
        } else if is_piped {
            // Tab-delimited output for piping: id, title, url, rrf_score
            for result in &results {
                println!(
                    "{}\t{}\t{}\t{:.6}",
                    result.bookmark.id.unwrap_or(0),
                    result.bookmark.title,
                    result.bookmark.url,
                    result.rrf_score
                );
            }
        } else {
            // Interactive colored output
            for result in &results {
                writeln!(
                    stderr,
                    "{} {} [{}] (score: {:.4})",
                    result
                        .bookmark
                        .id
                        .map_or("?".to_string(), |id| id.to_string())
                        .blue(),
                    result.bookmark.title.clone().green(),
                    result.bookmark.formatted_tags().yellow(),
                    format!("{:.4}", result.rrf_score).cyan()
                )?;
                writeln!(stderr, "  {}", result.bookmark.url)?;
                if !result.bookmark.description.is_empty() {
                    writeln!(stderr, "  {}", result.bookmark.description)?;
                }
                writeln!(stderr)?;
            }
        }

        writeln!(stderr, "{} bookmarks found", results.len())?;

        // Interactive prompt (same pattern as sem-search)
        if !non_interactive && !is_piped && !is_json && !results.is_empty() {
            use crate::util::helper::confirm;
            use std::io;

            if confirm("Open bookmark(s)?") {
                print!("Enter ID(s) to open (comma-separated): ");
                io::stdout()
                    .flush()
                    .cli_context("flushing stdout after prompt")?;

                let mut input = String::new();
                io::stdin()
                    .read_line(&mut input)
                    .cli_context("reading user input for bookmark IDs")?;

                let ids: Vec<i32> = input
                    .trim()
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();

                for id in ids {
                    if let Ok(Some(bookmark)) = services.bookmark_service.get_bookmark(id) {
                        crate::cli::process::execute_bookmark_default_action(
                            &bookmark,
                            services.action_service.clone(),
                        )?;
                    }
                }
            }
        }
    } else {
        return Err(CliError::CommandFailed(
            "Expected HSearch command".to_string(),
        ));
    }

    Ok(())
}
