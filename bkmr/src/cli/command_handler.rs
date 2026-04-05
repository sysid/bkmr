use crate::cli::args::{Cli, Commands};
use crate::cli::display::{show_bookmarks, DisplayBookmark, DisplayField};
use crate::cli::error::{CliError, CliResult};
use crate::cli::fzf::fzf_process;
use crate::cli::process::execute_bookmark_default_action;
use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::query::{BookmarkQuery, SortCriteria, SortDirection, SortField};
use crate::domain::system_tag::SystemTag;
use crate::infrastructure::di::ServiceContainer;
use crate::infrastructure::json::{write_bookmarks_as_json, JsonBookmarkView};
use crate::util::argument_processor::ArgumentProcessor;
use crate::util::helper::create_shell_function_name;
use crossterm::style::Stylize;
use itertools::Itertools;
use std::io::Write;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tracing::{instrument, warn};

/// Determine sort criteria from CLI flags.
///
/// Rules:
/// - No flags: Id ascending (default)
/// - `-o`/`-O` alone: imply Modified field (backward compat)
/// - `--sort <field>` alone: field's natural default (id/title: Asc, modified: Desc)
/// - `--sort <field>` + `-o`/`-O`: explicit combo
fn determine_sort_criteria(
    sort_field: Option<&str>,
    order_desc: bool,
    order_asc: bool,
) -> CliResult<SortCriteria> {
    let field = match sort_field {
        Some(f) => match f {
            "id" => SortField::Id,
            "title" => SortField::Title,
            "modified" => SortField::Modified,
            _ => {
                return Err(CliError::InvalidInput(format!(
                    "Invalid sort field '{}'. Valid values: id, title, modified",
                    f
                )))
            }
        },
        None => {
            // No --sort: if -o/-O given, imply Modified (backward compat)
            if order_desc || order_asc {
                SortField::Modified
            } else {
                return Ok(SortCriteria::new(SortField::Id, SortDirection::Ascending));
            }
        }
    };

    let direction = match (order_desc, order_asc) {
        (true, _) => SortDirection::Descending,
        (false, true) => SortDirection::Ascending,
        _ => match field {
            // Natural defaults: modified defaults to descending, others to ascending
            SortField::Modified => SortDirection::Descending,
            _ => SortDirection::Ascending,
        },
    };

    Ok(SortCriteria::new(field, direction))
}

/// Handler for search command and its sub-operations
pub struct SearchCommandHandler {
    services: ServiceContainer,
    settings: crate::config::Settings,
}

impl SearchCommandHandler {
    /// Create handler with dependency injection (single composition root)
    pub fn with_services(
        service_container: ServiceContainer,
        settings: crate::config::Settings,
    ) -> Self {
        Self {
            services: service_container,
            settings,
        }
    }

    /// Process search parameters and create query
    fn build_search_query(
        &self,
        fts_query: Option<String>,
        tags_exact: Option<String>,
        tags_exact_prefix: Option<String>,
        tags_all: Option<String>,
        tags_all_prefix: Option<String>,
        tags_all_not: Option<String>,
        tags_all_not_prefix: Option<String>,
        tags_any: Option<String>,
        tags_any_prefix: Option<String>,
        tags_any_not: Option<String>,
        tags_any_not_prefix: Option<String>,
        order_desc: bool,
        order_asc: bool,
        sort_field: Option<String>,
        limit: Option<i32>,
    ) -> CliResult<BookmarkQuery> {
        // Process all tag parameters using centralized logic
        let search_tags = ArgumentProcessor::process_search_tag_parameters(
            &tags_exact,
            &tags_exact_prefix,
            &tags_all,
            &tags_all_prefix,
            &tags_all_not,
            &tags_all_not_prefix,
            &tags_any,
            &tags_any_prefix,
            &tags_any_not,
            &tags_any_not_prefix,
        );

        // Determine sort criteria
        let sort_criteria =
            determine_sort_criteria(sort_field.as_deref(), order_desc, order_asc)?;

        // Validate and convert limit
        let limit_usize = match limit {
            Some(l) if l <= 0 => {
                return Err(CliError::InvalidInput(
                    "Limit must be a positive integer".to_string(),
                ))
            }
            Some(l) => Some(l as usize),
            None => None,
        };

        // Create query object
        let query = BookmarkQuery::new()
            .with_text_query(fts_query.as_deref())
            .with_tags_exact(search_tags.exact_tags.as_ref())
            .with_tags_all(search_tags.all_tags.as_ref())
            .with_tags_all_not(search_tags.all_not_tags.as_ref())
            .with_tags_any(search_tags.any_tags.as_ref())
            .with_tags_any_not(search_tags.any_not_tags.as_ref())
            .with_sort(sort_criteria)
            .with_limit(limit_usize);

        Ok(query)
    }

    /// Apply interpolation to bookmarks if requested
    fn apply_interpolation(&self, bookmarks: &mut [Bookmark]) -> CliResult<()> {
        for bookmark in bookmarks {
            if bookmark.url.contains("{{") || bookmark.url.contains("{%") {
                match self
                    .services
                    .interpolation_service
                    .render_bookmark_url(bookmark)
                {
                    Ok(rendered_url) => {
                        bookmark.url = rendered_url;
                    }
                    Err(e) => {
                        // Log error but continue with original content
                        warn!(
                            "Failed to interpolate bookmark {}: {}",
                            bookmark.id.unwrap_or(0),
                            e
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle different output modes for search results
    fn handle_output_mode(
        &self,
        bookmarks: &[Bookmark],
        is_fuzzy: bool,
        is_json: bool,
        fzf_style: Option<String>,
        fields: &[DisplayField],
        non_interactive: bool,
        stdout: bool,
        stderr: &mut StandardStream,
    ) -> CliResult<()> {
        match (is_fuzzy, is_json) {
            (true, _) => {
                let style = fzf_style.as_deref().unwrap_or("classic");
                fzf_process(bookmarks, style, &self.services, &self.settings, stdout)?;
            }
            (_, true) => {
                let json_views = JsonBookmarkView::from_domain_collection(bookmarks);
                write_bookmarks_as_json(&json_views)?;
            }
            _ => {
                self.display_search_results(stderr, bookmarks, fields, non_interactive)?;
            }
        }
        Ok(())
    }

    /// Display search results in normal mode
    #[instrument(skip(self, stderr, bookmarks, fields), level = "debug")]
    fn display_search_results(
        &self,
        stderr: &mut StandardStream,
        bookmarks: &[Bookmark],
        fields: &[DisplayField],
        non_interactive: bool,
    ) -> CliResult<()> {
        // If there's exactly one result and we're in interactive mode, execute the default action directly
        if bookmarks.len() == 1 && !non_interactive {
            let bookmark = &bookmarks[0];
            writeln!(
                stderr,
                "Found 1 bookmark: {} (ID: {}). Executing default action...",
                bookmark.title.clone().green(),
                bookmark.id.unwrap_or(0)
            )?;

            return execute_bookmark_default_action(bookmark, self.services.action_service.clone());
        }

        // Convert to display bookmarks
        let display_bookmarks: Vec<DisplayBookmark> =
            bookmarks.iter().map(DisplayBookmark::from_domain).collect();

        show_bookmarks(&display_bookmarks, fields, &self.settings);
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
            use crate::cli::process::process;
            use crate::domain::error_context::CliErrorContext;

            stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                .cli_context("Failed to set color")?;
            writeln!(stderr, "Selection: ").cli_context("Failed to write to stderr")?;
            stderr.reset().cli_context("Failed to reset color")?;

            process(bookmarks, &self.services, &self.settings)?;
        }
        Ok(())
    }
}

impl SearchCommandHandler {
    #[instrument(skip(self, cli))]
    pub fn execute(&self, cli: Cli) -> CliResult<()> {
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
            sort_field,
            non_interactive,
            is_fuzzy,
            fzf_style,
            is_json,
            limit,
            interpolate,
            shell_stubs,
            stdout,
            embeddable,
        } = cli.command.unwrap()
        {
            let mut fields = crate::cli::display::DEFAULT_FIELDS.to_vec();

            // Add timestamp field if sorting by modified date
            let is_modified_sort = sort_field.as_deref() == Some("modified")
                || (sort_field.is_none() && (order_desc || order_asc));
            if is_modified_sort {
                fields.push(DisplayField::LastUpdateTs);
            }

            // Build search query
            let query = self.build_search_query(
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
                sort_field,
                limit,
            )?;

            // Execute search
            let mut bookmarks = self.services.bookmark_service.search_bookmarks(&query)?;

            // Filter to embeddable only if requested
            if embeddable {
                bookmarks.retain(|b| b.embeddable);
            }

            // Apply interpolation if requested
            if interpolate {
                self.apply_interpolation(&mut bookmarks)?;
            }

            // Handle shell stubs mode
            if shell_stubs {
                return self.output_shell_stubs(&bookmarks);
            }

            // Handle output mode
            let mut stderr = termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto);
            self.handle_output_mode(
                &bookmarks,
                is_fuzzy,
                is_json,
                fzf_style,
                &fields,
                non_interactive,
                stdout,
                &mut stderr,
            )?;
        }
        Ok(())
    }
}

impl SearchCommandHandler {
    /// Output shell function stubs for shell script bookmarks
    fn output_shell_stubs(&self, bookmarks: &[Bookmark]) -> CliResult<()> {
        // Filter for shell script bookmarks
        let shell_bookmarks: Vec<&Bookmark> = bookmarks
            .iter()
            .filter(|bookmark| {
                bookmark
                    .tags
                    .iter()
                    .any(|tag| tag.is_system_tag_of(SystemTag::Shell))
            })
            .collect();

        // Generate shell function stubs
        for bookmark in shell_bookmarks {
            if let Some(id) = bookmark.id {
                // Create a valid shell function name from the bookmark title
                let function_name = create_shell_function_name(&bookmark.title);

                // Output the shell function
                println!(
                    "{}() {{ bkmr open --no-edit {} -- \"$@\"; }}",
                    function_name, id
                );
                println!("export -f {}", function_name);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_no_flags_when_determine_sort_criteria_then_returns_id_ascending() {
        let result = determine_sort_criteria(None, false, false).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Id, SortDirection::Ascending));
    }

    #[test]
    fn given_desc_flag_only_when_determine_sort_criteria_then_returns_modified_descending() {
        let result = determine_sort_criteria(None, true, false).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Modified, SortDirection::Descending));
    }

    #[test]
    fn given_asc_flag_only_when_determine_sort_criteria_then_returns_modified_ascending() {
        let result = determine_sort_criteria(None, false, true).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Modified, SortDirection::Ascending));
    }

    #[test]
    fn given_both_direction_flags_when_determine_sort_criteria_then_desc_wins() {
        let result = determine_sort_criteria(None, true, true).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Modified, SortDirection::Descending));
    }

    #[test]
    fn given_sort_title_when_determine_sort_criteria_then_returns_title_ascending() {
        let result = determine_sort_criteria(Some("title"), false, false).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Title, SortDirection::Ascending));
    }

    #[test]
    fn given_sort_title_desc_when_determine_sort_criteria_then_returns_title_descending() {
        let result = determine_sort_criteria(Some("title"), true, false).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Title, SortDirection::Descending));
    }

    #[test]
    fn given_sort_modified_when_determine_sort_criteria_then_returns_modified_descending() {
        let result = determine_sort_criteria(Some("modified"), false, false).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Modified, SortDirection::Descending));
    }

    #[test]
    fn given_sort_modified_asc_when_determine_sort_criteria_then_returns_modified_ascending() {
        let result = determine_sort_criteria(Some("modified"), false, true).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Modified, SortDirection::Ascending));
    }

    #[test]
    fn given_sort_id_desc_when_determine_sort_criteria_then_returns_id_descending() {
        let result = determine_sort_criteria(Some("id"), true, false).unwrap();
        assert_eq!(result, SortCriteria::new(SortField::Id, SortDirection::Descending));
    }

    #[test]
    fn given_invalid_sort_field_when_determine_sort_criteria_then_returns_error() {
        let result = determine_sort_criteria(Some("invalid"), false, false);
        assert!(result.is_err());
    }
}
