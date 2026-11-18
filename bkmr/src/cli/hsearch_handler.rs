// src/cli/hsearch_handler.rs
use std::io::Write;

use crossterm::style::Stylize;

use crate::cli::args::{Cli, Commands};
use crate::cli::error::{CliError, CliResult};
use crate::cli::fzf::fzf_process;
use crate::config::Settings;
use crate::domain::bookmark::Bookmark;
use crate::domain::error_context::CliErrorContext;
use crate::domain::search::{HybridSearch, HybridSearchResult, SearchMode};
use crate::domain::tag::Tag;
use crate::infrastructure::di::ServiceContainer;
use crate::infrastructure::json::{write_bookmarks_as_json, JsonSearchHit};
use crate::util::helper::is_stdout_piped;
use tracing::instrument;

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
    if tags.is_empty() {
        None
    } else {
        Some(tags)
    }
}

/// Tab-delimited line for piping: id, title, url, rrf_score, similarity, keyword_match.
///
/// Always six columns, with empty cells where a search did not run or did not return the hit,
/// so the layout does not depend on the mode. The first four columns predate the last two.
fn piped_line(result: &HybridSearchResult) -> String {
    format!(
        "{}\t{}\t{}\t{:.6}\t{}\t{}",
        result.bookmark.id.unwrap_or(0),
        result.bookmark.title,
        result.bookmark.url,
        result.rrf_score,
        result
            .similarity
            .map_or(String::new(), |s| format!("{:.3}", s)),
        result
            .keyword_match
            .map_or("", |matched| if matched { "1" } else { "0" }),
    )
}

/// Shows where a hit came from: `[kw]` for a keyword match, `sim N.NNN` for a semantic one.
fn match_marker(result: &HybridSearchResult) -> String {
    let keyword = (result.keyword_match == Some(true)).then_some("[kw]".to_string());
    let similarity = result.similarity.map(|s| format!("sim {:.3}", s));
    [keyword, similarity]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

#[instrument(skip(cli, services, settings), level = "debug")]
pub fn hybrid_search(cli: Cli, services: &ServiceContainer, settings: &Settings) -> CliResult<()> {
    let mut stderr = std::io::stderr();
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
        is_fuzzy,
        stdout,
        non_interactive,
    } = cli.command.unwrap()
    {
        // Build the hybrid search query
        let mut search = HybridSearch::new(query);
        search.limit = limit.map(|l| l as usize);
        // clap's PossibleValuesParser admits only these values
        search.mode = match mode.as_str() {
            "exact" => SearchMode::Exact,
            "semantic" => SearchMode::Semantic,
            _ => SearchMode::Hybrid,
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

        if search.mode == SearchMode::Semantic
            && !services
                .vector_repository
                .has_embeddings()
                .cli_context("checking for stored embeddings")?
        {
            writeln!(
                stderr,
                "{}",
                "No embeddings yet, so semantic search finds nothing. Run `bkmr backfill`."
                    .yellow()
            )
            .cli_context("writing missing-embeddings hint to stderr")?;
        }

        // JSON mode falls through so zero hits serialize as `[]`, matching `search --json`.
        if results.is_empty() && !is_json {
            writeln!(stderr, "{}", "No bookmarks found".yellow())
                .cli_context("writing empty search result to stderr")?;
            return Ok(());
        }

        let is_piped = is_stdout_piped();

        // JSON wins when combined with the picker: --fzf was ignored when --json
        // was set before this change, and scripts may rely on that.
        if is_fuzzy && !is_json {
            let bookmarks: Vec<Bookmark> = results
                .iter()
                .map(|result| result.bookmark.clone())
                .collect();
            fzf_process(&bookmarks, "classic", services, settings, stdout)?;
            return Ok(());
        }

        if is_json {
            let hits: Vec<JsonSearchHit> = results.iter().map(JsonSearchHit::from_domain).collect();
            write_bookmarks_as_json(&hits).cli_context("writing search results as JSON")?;
        } else if is_piped {
            for result in &results {
                println!("{}", piped_line(result));
            }
        } else {
            // Interactive colored output
            for result in &results {
                writeln!(
                    stderr,
                    "{} {} [{}] (score: {:.4}) {}",
                    result
                        .bookmark
                        .id
                        .map_or("?".to_string(), |id| id.to_string())
                        .blue(),
                    result.bookmark.title.clone().green(),
                    result.bookmark.formatted_tags().yellow(),
                    format!("{:.4}", result.rrf_score).cyan(),
                    match_marker(result).magenta()
                )?;
                writeln!(stderr, "  {}", result.bookmark.url)?;
                if !result.bookmark.description.is_empty() {
                    writeln!(stderr, "  {}", result.bookmark.description)?;
                }
                writeln!(stderr)?;
            }
        }

        writeln!(stderr, "{} bookmarks found", results.len())?;

        // Interactive prompt
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn result(similarity: Option<f64>, keyword_match: Option<bool>) -> HybridSearchResult {
        let mut bookmark =
            Bookmark::new("https://example.com", "Example", "", HashSet::new()).unwrap();
        bookmark.id = Some(7);
        HybridSearchResult::new(bookmark, 0.016393, similarity, keyword_match)
    }

    #[test]
    fn given_hit_from_both_searches_when_piped_then_six_columns_with_scores() {
        let line = piped_line(&result(Some(0.8123), Some(true)));

        assert_eq!(line, "7\tExample\thttps://example.com\t0.016393\t0.812\t1");
    }

    #[test]
    fn given_meaning_only_hit_when_piped_then_keyword_column_is_zero() {
        let line = piped_line(&result(Some(0.68), Some(false)));

        assert!(line.ends_with("\t0.680\t0"), "{line}");
    }

    #[test]
    fn given_hit_without_scores_when_piped_then_still_six_columns_with_empty_cells() {
        // The column count must not depend on the mode, so scripts can rely on it
        let line = piped_line(&result(None, None));

        assert_eq!(line.split('\t').count(), 6, "{line}");
        assert!(line.ends_with("\t0.016393\t\t"), "{line}");
    }

    #[test]
    fn given_hits_when_marked_then_shows_keyword_flag_and_similarity() {
        assert_eq!(
            match_marker(&result(Some(0.7123), Some(true))),
            "[kw] sim 0.712"
        );
        assert_eq!(match_marker(&result(Some(0.64), Some(false))), "sim 0.640");
        assert_eq!(match_marker(&result(None, Some(true))), "[kw]");
        assert_eq!(match_marker(&result(None, None)), "");
    }
}
