// src/cli/tag_commands.rs
use crate::cli::args::{Cli, Commands};
use crate::cli::error::CliResult;
use crate::domain::tag::Tag;
use crate::util::helper::is_stdout_piped;
use crate::infrastructure::di::ServiceContainer;
use crossterm::style::Stylize;

pub fn show_tags(cli: Cli, services: &ServiceContainer) -> CliResult<()> {
    if let Commands::Tags { tag } = cli.command.unwrap() {
        let tag_service = &services.tag_service;

        // Determine if stdout is being piped to another process
        let is_piped = is_stdout_piped();

        match tag {
            Some(tag_str) => {
                // Show related tags for the specified tag
                let parsed_tag = Tag::new(&tag_str)?;
                let related_tags = tag_service.get_related_tags(&parsed_tag)?;

                if related_tags.is_empty() {
                    eprintln!("No related tags found for '{}'", tag_str.blue());
                } else {
                    eprintln!("Tags related to '{}':", tag_str.blue());

                    // Sort by count (most frequent first)
                    let mut sorted_tags = related_tags;
                    sorted_tags.sort_by(|(_, count_a), (_, count_b)| count_b.cmp(count_a));

                    // let output = String::new();
                    for (tag, count) in sorted_tags {
                        if is_piped {
                            // Plain text for piping
                            println!("{} ({})", tag.value(), count);
                        } else {
                            // Colored text for terminal
                            println!("{} ({})", tag.value().green(), count);
                        }
                    }
                }
            }
            None => {
                // Show all tags
                let all_tags = tag_service.get_all_tags()?;

                if all_tags.is_empty() {
                    eprintln!("No tags found");
                } else {
                    eprintln!("All tags:");

                    // Sort by count (most frequent first)
                    let mut sorted_tags = all_tags;
                    sorted_tags.sort_by(|(_, count_a), (_, count_b)| count_b.cmp(count_a));

                    // let output = String::new();
                    for (tag, count) in sorted_tags {
                        if is_piped {
                            // Plain text for piping
                            println!("{} ({})", tag.value(), count);
                        } else {
                            // Colored text for terminal
                            println!("{} ({})", tag.value().green(), count);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Tests would be added here
}
