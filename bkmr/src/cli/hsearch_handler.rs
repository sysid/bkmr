// src/cli/hsearch_handler.rs
use std::io::Write;

use crossterm::style::Stylize;

use crate::cli::args::{Cli, Commands};
use crate::cli::bookmark_commands::get_ids;
use crate::cli::error::{CliError, CliResult};
use crate::cli::fzf::fzf_process;
use crate::config::Settings;
use crate::domain::bookmark::Bookmark;
use crate::domain::error_context::CliErrorContext;
use crate::domain::search::{HybridSearch, HybridSearchResult, SearchMode};
use crate::infrastructure::di::ServiceContainer;
use crate::infrastructure::json::{write_bookmarks_as_json, JsonSearchHit};
use crate::util::argument_processor::ArgumentProcessor;
use crate::util::helper::is_stdout_piped;
use tracing::instrument;

/// Tab-delimited line for piping: id, title, url, rrf_score, similarity, keyword_match.
///
/// Always one line of six columns, with empty cells where a search did not run or did not return
/// the hit, so the layout does not depend on the mode. The first four columns predate the last
/// two. Title and url are escaped, since snippet and markdown content spans lines.
fn piped_line(result: &HybridSearchResult) -> String {
    format!(
        "{}\t{}\t{}\t{:.6}\t{}\t{}",
        result.bookmark.id.unwrap_or(0),
        escape_cell(&result.bookmark.title),
        escape_cell(&result.bookmark.url),
        result.rrf_score,
        result
            .similarity
            .map_or(String::new(), |s| format!("{:.3}", s)),
        result
            .keyword_match
            .map_or("", |matched| if matched { "1" } else { "0" }),
    )
}

/// Escapes `\`, tab, newline and carriage return as `\\`, `\t`, `\n`, `\r`, so a cell cannot
/// break the line or column layout and the original text can be recovered.
fn escape_cell(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Bookmarks the user picked at the open prompt, by ascending ID. Checks every ID before any is
/// opened: a typo or an ID from outside the results fails instead of being skipped.
fn selected_bookmarks<'a>(
    input: &str,
    results: &'a [HybridSearchResult],
) -> CliResult<Vec<&'a Bookmark>> {
    if input.trim().is_empty() {
        return Ok(vec![]);
    }
    get_ids(input.trim().to_string())?
        .into_iter()
        .map(|id| {
            results
                .iter()
                .find(|r| r.bookmark.id == Some(id))
                .map(|r| &r.bookmark)
                .ok_or_else(|| {
                    CliError::InvalidInput(format!("Bookmark ID {} is not among the results", id))
                })
        })
        .collect()
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
        tags,
        mode,
        limit,
        is_json,
        fzf,
        stdout,
        non_interactive,
    } = cli.command.unwrap()
    {
        // Build the hybrid search query
        let mut search = HybridSearch::new(query);
        search.limit = limit.map(|l| l as usize);
        search.mode = mode.into();

        // Same tag semantics as `search`, including the prefixes shell wrappers rely on
        let tags = ArgumentProcessor::process_search_tag_parameters(&tags)?;
        search.tags_exact = tags.exact_tags;
        search.tags_all = tags.all_tags;
        search.tags_all_not = tags.all_not_tags;
        search.tags_any = tags.any_tags;
        search.tags_any_not = tags.any_not_tags;

        // Perform hybrid search
        let outcome = services
            .bookmark_service
            .hybrid_search(&search)
            .cli_context("performing hybrid search on bookmarks")?;
        let results = outcome.hits;

        // Hybrid mode falls back to keyword matches without vectors, as its help says
        if search.mode == SearchMode::Semantic && outcome.missing_embeddings {
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
        if fzf.is_fuzzy && !is_json {
            let bookmarks: Vec<Bookmark> = results
                .iter()
                .map(|result| result.bookmark.clone())
                .collect();
            fzf_process(
                &bookmarks,
                fzf.fzf_style.as_deref().unwrap_or("classic"),
                // Keep the RRF ranking while typing; typing only filters.
                // Revisit if users miss skim's match-score sorting here.
                true,
                services,
                settings,
                stdout,
            )?;
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

                for bookmark in selected_bookmarks(&input, &results)? {
                    crate::cli::process::execute_bookmark_default_action(
                        bookmark,
                        services.action_service.clone(),
                    )?;
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
    fn given_tabs_and_newlines_in_title_and_content_when_piped_then_one_line_of_six_columns() {
        let mut bookmark = Bookmark::new(
            "line one\n\tline two\r\nC:\\path",
            "a\ttitle",
            "",
            HashSet::new(),
        )
        .unwrap();
        bookmark.id = Some(7);
        let hit = HybridSearchResult::new(bookmark, 0.016393, None, Some(true));

        let line = piped_line(&hit);

        assert_eq!(line.lines().count(), 1, "{line}");
        assert_eq!(line.split('\t').count(), 6, "{line}");
        // Backslash is escaped too, so a literal `\n` in the text stays distinguishable
        assert!(
            line.starts_with("7\ta\\ttitle\tline one\\n\\tline two\\r\\nC:\\\\path\t"),
            "{line}"
        );
    }

    #[test]
    fn given_ids_from_results_when_selecting_then_returns_those_bookmarks_by_ascending_id() {
        let mut other = result(None, Some(true));
        other.bookmark.id = Some(9);
        let results = vec![other, result(None, Some(true))];

        let selected = selected_bookmarks(" 9, 7 \n", &results).unwrap();

        let ids: Vec<Option<i32>> = selected.iter().map(|b| b.id).collect();
        assert_eq!(ids, vec![Some(7), Some(9)]);
    }

    #[test]
    fn given_empty_input_when_selecting_then_nothing_is_selected() {
        let results = vec![result(None, Some(true))];

        assert!(selected_bookmarks("  \n", &results).unwrap().is_empty());
    }

    #[test]
    fn given_non_numeric_id_when_selecting_then_error_and_nothing_is_selected() {
        let results = vec![result(None, Some(true))];

        let error = selected_bookmarks("7,abc", &results).unwrap_err();

        assert!(
            matches!(error, CliError::InvalidIdFormat(_)),
            "got: {error:?}"
        );
    }

    #[test]
    fn given_id_not_among_results_when_selecting_then_error_names_the_id() {
        let results = vec![result(None, Some(true))];

        let error = selected_bookmarks("7,999", &results).unwrap_err();

        assert!(matches!(error, CliError::InvalidInput(_)), "got: {error:?}");
        assert!(error.to_string().contains("999"), "{error}");
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
