// src/cli/fzf.rs

use std::process::{Command, Stdio};
use std::io::Write;
use crate::application::dto::BookmarkResponse;
use crate::cli::error::{CliError, CliResult};
use crate::environment::CONFIG;

pub fn fzf_process(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    if bookmarks.is_empty() {
        println!("No bookmarks to display");
        return Ok(());
    }

    // Format bookmarks for fzf
    let mut bookmark_lines = Vec::new();
    for bm in bookmarks {
        let id = bm.id.unwrap_or(0);
        let title = &bm.title;
        let url = &bm.url;
        let formatted_tags = bm.tags.join(",");

        // Format tags display based on config
        let tags_display = if CONFIG.fzf_opts.show_tags {
            format!(" [{}]", formatted_tags)
        } else {
            String::new()
        };

        // Format line based on URL display preference
        let line = if CONFIG.fzf_opts.no_url {
            format!("{}: {}{}", id, title, tags_display)
        } else {
            format!("{}: {} <{}>{}", id, title, url, tags_display)
        };

        bookmark_lines.push(line);
    }

    // Build fzf arguments
    let mut fzf_args = vec![
        "--multi",
        "--reverse",
        "--inline-info",
        &format!("--height={}", CONFIG.fzf_opts.height),
        "--bind=ctrl-o:execute(echo open {})+abort",
        "--bind=ctrl-e:execute(echo edit {})+abort",
        "--bind=enter:execute(echo open {})+abort",
    ];

    if CONFIG.fzf_opts.reverse {
        fzf_args.push("--tac");
    }

    // Execute fzf command
    let mut child = Command::new("fzf")
        .args(&fzf_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| CliError::CommandFailed(format!("Failed to spawn fzf: {}", e)))?;

    // Write bookmark lines to fzf stdin
    if let Some(mut stdin) = child.stdin.take() {
        for line in bookmark_lines {
            writeln!(stdin, "{}", line)
                .map_err(|e| CliError::CommandFailed(format!("Failed to write to fzf: {}", e)))?;
        }
    }

    // Get fzf output
    let output = child.wait_with_output()
        .map_err(|e| CliError::CommandFailed(format!("Failed to get fzf output: {}", e)))?;

    if !output.stdout.is_empty() {
        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines() {
            if line.starts_with("open ") || line.starts_with("edit ") {
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                let action = parts[0];
                let id_str = parts.get(1).unwrap_or(&"");

                // Extract bookmark ID from the line
                if let Some(id_end) = id_str.find(':') {
                    if let Ok(id) = id_str[..id_end].parse::<i32>() {
                        match action {
                            "open" => {
                                if let Some(bookmark) = bookmarks.iter().find(|b| b.id() == Some(id)) {
                                    // Convert domain bookmark to DTO for open_bookmark
                                    let bookmark_dto = crate::application::dto::BookmarkResponse::from_domain(bookmark);
                                    crate::cli::process::open_bookmark(&bookmark_dto)?;
                                }
                            },
                            "edit" => {
                                if let Some(bookmark) = bookmarks.iter().find(|b| b.id() == Some(id)) {
                                    let bookmarks_to_edit = vec![bookmark.clone()];
                                    crate::cli::process::edit_bookmarks(vec![id], bookmarks_to_edit)?;
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
