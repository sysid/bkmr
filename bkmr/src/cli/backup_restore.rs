// src/cli/backup_restore.rs
//! `restore --from FILE`: bring back changed and deleted bookmarks from a JSON backup.
//!
//! The whole plan is worked out before anything is written, so the user sees every conflict
//! up front and decides on complete information: confirm, pick a subset, or look at diffs first.

use crate::application::services::bookmark_service::{
    BackupRestoreItem, BackupRestoreState, BookmarkService,
};
use crate::cli::error::{CliError, CliResult};
use crate::cli::fzf::{pick_restore_ids, RestorePickItem};
use crate::cli::history_diff::{render_patch, MemoryState};
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use tracing::instrument;

/// What the caller of `restore --from` asked for
pub struct BackupRestoreRequest<'a> {
    pub path: &'a str,
    /// Empty: every restorable entry
    pub ids: &'a [i32],
    pub yes: bool,
    pub dry_run: bool,
    pub is_json: bool,
    pub color: bool,
}

#[instrument(skip(request, bookmark_service), fields(path = request.path))]
pub fn restore_from_backup(
    request: &BackupRestoreRequest,
    bookmark_service: Arc<dyn BookmarkService>,
) -> CliResult<()> {
    let plan = bookmark_service.plan_backup_restore(request.path)?;
    let selection = select(&plan, request.ids)?;

    if request.is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&plan_json(&selection))
                .unwrap_or_else(|_| "[]".to_string())
        );
        return Ok(());
    }

    let restorable: Vec<&BackupRestoreItem> = selection
        .iter()
        .copied()
        .filter(|item| item.is_restorable())
        .collect();
    // An explicitly requested ID that cannot be restored is a failure; with no IDs given,
    // such entries were never asked for and are only reported
    let blocked: Vec<i32> = if request.ids.is_empty() {
        Vec::new()
    } else {
        selection
            .iter()
            .filter(|item| !item.is_restorable() && !is_unchanged(item))
            .filter_map(|item| item.id)
            .collect()
    };

    print!("{}", plan_table(request.path, &plan, &selection));

    if request.dry_run {
        for item in &restorable {
            print!("{}", item_patch(item, request.color));
        }
        return Ok(());
    }

    if restorable.is_empty() {
        eprintln!("Nothing to restore");
        return report_blocked(&blocked);
    }

    let chosen = if request.yes {
        restorable
    } else {
        match ask(&restorable, request.color)? {
            Some(chosen) => chosen,
            None => {
                eprintln!("Restore cancelled");
                return Err(CliError::OperationAborted);
            }
        }
    };

    let mut failed = Vec::new();
    let (mut updated, mut readded) = (0, 0);
    for item in chosen {
        let id = item.id.unwrap_or(0);
        match bookmark_service.apply_backup_restore(item) {
            Ok(_) => match item.state {
                BackupRestoreState::Deleted { .. } => {
                    readded += 1;
                    eprintln!("Re-added #{} {}", id, item.title);
                }
                _ => {
                    updated += 1;
                    eprintln!("Restored #{} {}", id, item.title);
                }
            },
            Err(e) => {
                eprintln!("Error restoring #{}: {}", id, e);
                failed.push(id);
            }
        }
    }
    eprintln!(
        "Restored {} bookmarks: {} updated, {} re-added",
        updated + readded,
        updated,
        readded
    );

    if !failed.is_empty() {
        return Err(CliError::CommandFailed(format!(
            "Failed to restore bookmarks with IDs: {:?}",
            failed
        )));
    }
    report_blocked(&blocked)
}

fn report_blocked(blocked: &[i32]) -> CliResult<()> {
    if blocked.is_empty() {
        return Ok(());
    }
    Err(CliError::CommandFailed(format!(
        "Not restorable, see the plan above: {:?}",
        blocked
    )))
}

/// The plan entries the user asked for: those with the given IDs, or all of them
fn select<'a>(plan: &'a [BackupRestoreItem], ids: &[i32]) -> CliResult<Vec<&'a BackupRestoreItem>> {
    if ids.is_empty() {
        return Ok(plan.iter().collect());
    }
    let in_backup: HashSet<i32> = plan.iter().filter_map(|item| item.id).collect();
    let missing: Vec<i32> = ids
        .iter()
        .copied()
        .filter(|id| !in_backup.contains(id))
        .collect();
    if !missing.is_empty() {
        return Err(CliError::InvalidInput(format!(
            "Not in the backup: {:?}",
            missing
        )));
    }
    let wanted: HashSet<i32> = ids.iter().copied().collect();
    Ok(plan
        .iter()
        .filter(|item| item.id.is_some_and(|id| wanted.contains(&id)))
        .collect())
}

fn is_unchanged(item: &BackupRestoreItem) -> bool {
    matches!(item.state, BackupRestoreState::Unchanged)
}

fn state_name(state: &BackupRestoreState) -> &'static str {
    match state {
        BackupRestoreState::Changed { .. } => "changed",
        BackupRestoreState::Deleted { .. } => "deleted",
        BackupRestoreState::Conflict { .. } => "conflict",
        BackupRestoreState::FileBacked { .. } => "file",
        BackupRestoreState::Unchanged => "unchanged",
        BackupRestoreState::NoId => "no-id",
    }
}

/// What the last column of the plan says about an entry
fn state_detail(state: &BackupRestoreState) -> String {
    match state {
        BackupRestoreState::Changed { fields, .. } => fields.join(", "),
        BackupRestoreState::Conflict { blocking_id } => {
            format!("URL/content now used by #{}", blocking_id)
        }
        BackupRestoreState::FileBacked { file_path } => {
            format!("imported from {}: change the file instead", file_path)
        }
        _ => String::new(),
    }
}

/// Header line with counts, then one line per entry worth a look: everything except unchanged
/// entries and entries without ID, which are only counted
fn plan_table(path: &str, plan: &[BackupRestoreItem], selection: &[&BackupRestoreItem]) -> String {
    let unchanged = selection.iter().filter(|item| is_unchanged(item)).count();
    let without_id = selection
        .iter()
        .filter(|item| matches!(item.state, BackupRestoreState::NoId))
        .count();
    let mut out = format!(
        "Backup {}: {} entries, {} unchanged",
        path,
        plan.len(),
        unchanged
    );
    if without_id > 0 {
        out.push_str(&format!(
            ", {} without ID (not from bkmr search --json, use load-json)",
            without_id
        ));
    }
    out.push('\n');

    let rows: Vec<&&BackupRestoreItem> = selection
        .iter()
        .filter(|item| !is_unchanged(item) && !matches!(item.state, BackupRestoreState::NoId))
        .collect();
    if rows.is_empty() {
        return out;
    }
    let id_width = rows
        .iter()
        .map(|item| item.id.unwrap_or(0).to_string().len())
        .max()
        .unwrap_or(2)
        .max(2);
    let title_width = rows
        .iter()
        .map(|item| item.title.chars().count())
        .max()
        .unwrap_or(5)
        .clamp(5, 40);
    out.push('\n');
    out.push_str(&format!(
        "  {:>iw$}  {:<9} {:<tw$}  {}\n",
        "ID",
        "STATE",
        "TITLE",
        "DETAIL",
        iw = id_width,
        tw = title_width
    ));
    for item in rows {
        out.push_str(
            format!(
                "  {:>iw$}  {:<9} {:<tw$}  {}",
                item.id.unwrap_or(0),
                state_name(&item.state),
                truncate(&item.title, title_width),
                state_detail(&item.state),
                iw = id_width,
                tw = title_width
            )
            .trim_end(),
        );
        out.push('\n');
    }
    out.push('\n');
    out
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(width.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// Unified diff from the bookmark as it is now to the backup's state
fn item_patch(item: &BackupRestoreItem, color: bool) -> String {
    let id = item.id.unwrap_or(0);
    let backup_label = format!("#{} backup", id);
    match &item.state {
        BackupRestoreState::Changed {
            current,
            restored,
            fields,
        } => {
            let patch = render_patch(
                Some(&MemoryState::from_bookmark(current)),
                Some(&MemoryState::from_bookmark(restored)),
                &format!("#{} current", id),
                &backup_label,
                color,
            );
            if patch.is_empty() {
                // The diff covers text fields only; opener and embeddable show up here
                format!("#{}: only {} differ\n", id, fields.join(", "))
            } else {
                patch
            }
        }
        BackupRestoreState::Deleted { restored } => render_patch(
            None,
            Some(&MemoryState::from_bookmark(restored)),
            "/dev/null",
            &backup_label,
            color,
        ),
        _ => String::new(),
    }
}

fn question(items: &[&BackupRestoreItem]) -> String {
    let deleted = items
        .iter()
        .filter(|item| matches!(item.state, BackupRestoreState::Deleted { .. }))
        .count();
    let changed = items.len() - deleted;
    format!(
        "Restore {} changed and {} deleted bookmarks?  [y]es  [n]o  [s]elect  [d]iff: ",
        changed, deleted
    )
}

/// Ask until the user decides. `None`: declined, cancelled, or stdin ended.
fn ask<'a>(
    restorable: &[&'a BackupRestoreItem],
    color: bool,
) -> CliResult<Option<Vec<&'a BackupRestoreItem>>> {
    loop {
        print!("{}", question(restorable));
        io::stdout().flush()?;
        let mut answer = String::new();
        if io::stdin().read_line(&mut answer)? == 0 {
            println!();
            return Ok(None);
        }
        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(Some(restorable.to_vec())),
            "d" | "diff" => {
                for item in restorable {
                    print!("{}", item_patch(item, color));
                }
                println!();
            }
            "s" | "select" => {
                if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                    eprintln!("Selecting needs a terminal; use --ids instead");
                    continue;
                }
                return pick(restorable);
            }
            _ => return Ok(None),
        }
    }
}

/// Multi-select in skim with the diff as preview; everything starts selected
fn pick<'a>(restorable: &[&'a BackupRestoreItem]) -> CliResult<Option<Vec<&'a BackupRestoreItem>>> {
    let items = restorable
        .iter()
        .map(|item| RestorePickItem {
            id: item.id.unwrap_or(0),
            display: format!(
                "{:>5}  {:<8} {}",
                item.id.unwrap_or(0),
                state_name(&item.state),
                item.title
            ),
            preview: item_patch(item, true),
        })
        .collect();
    let Some(ids) = pick_restore_ids(items)? else {
        return Ok(None);
    };
    let chosen: Vec<&BackupRestoreItem> = restorable
        .iter()
        .copied()
        .filter(|item| item.id.is_some_and(|id| ids.contains(&id)))
        .collect();
    Ok((!chosen.is_empty()).then_some(chosen))
}

/// The plan for `--dry-run --json`: one object per entry, for scripts and agents
fn plan_json(selection: &[&BackupRestoreItem]) -> Vec<serde_json::Value> {
    selection
        .iter()
        .map(|item| {
            let mut entry = serde_json::json!({
                "id": item.id,
                "title": item.title,
                "state": state_name(&item.state),
            });
            match &item.state {
                BackupRestoreState::Changed { fields, .. } => {
                    entry["changed_fields"] = serde_json::json!(fields);
                }
                BackupRestoreState::Conflict { blocking_id } => {
                    entry["blocking_id"] = serde_json::json!(blocking_id);
                }
                BackupRestoreState::FileBacked { file_path } => {
                    entry["file_path"] = serde_json::json!(file_path);
                }
                _ => {}
            }
            entry
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bookmark::Bookmark;
    use std::collections::HashSet;

    fn bookmark(id: i32, url: &str) -> Box<Bookmark> {
        let mut bookmark = Bookmark::new(url, "Title", "", HashSet::new()).unwrap();
        bookmark.set_id(id);
        Box::new(bookmark)
    }

    fn item(id: Option<i32>, title: &str, state: BackupRestoreState) -> BackupRestoreItem {
        BackupRestoreItem {
            id,
            title: title.to_string(),
            state,
        }
    }

    #[test]
    fn given_no_ids_when_selecting_then_whole_plan_is_selected() {
        let plan = vec![
            item(Some(1), "a", BackupRestoreState::Unchanged),
            item(None, "b", BackupRestoreState::NoId),
        ];

        let selection = select(&plan, &[]).unwrap();

        assert_eq!(selection.len(), 2);
    }

    #[test]
    fn given_id_missing_from_backup_when_selecting_then_invalid_input_names_it() {
        let plan = vec![item(Some(1), "a", BackupRestoreState::Unchanged)];

        let err = select(&plan, &[1, 7]).unwrap_err();

        assert!(matches!(err, CliError::InvalidInput(ref msg) if msg.contains('7')));
    }

    #[test]
    fn given_plan_when_rendering_table_then_unchanged_and_no_id_entries_are_only_counted() {
        let plan = vec![
            item(
                Some(12),
                "Deploy",
                BackupRestoreState::Changed {
                    current: bookmark(12, "a"),
                    restored: bookmark(12, "b"),
                    fields: vec!["url", "tags"],
                },
            ),
            item(
                Some(77),
                "Grafana",
                BackupRestoreState::Conflict { blocking_id: 903 },
            ),
            item(Some(5), "Quiet", BackupRestoreState::Unchanged),
            item(None, "Hand", BackupRestoreState::NoId),
        ];
        let selection: Vec<&BackupRestoreItem> = plan.iter().collect();

        let table = plan_table("b.json", &plan, &selection);

        assert!(table.starts_with("Backup b.json: 4 entries, 1 unchanged, 1 without ID"));
        assert!(
            table.contains("12  changed   Deploy   url, tags"),
            "{table}"
        );
        assert!(
            table.contains("conflict  Grafana  URL/content now used by #903"),
            "{table}"
        );
        assert!(!table.contains("Quiet"), "{table}");
        assert!(!table.contains("Hand"), "{table}");
    }

    #[test]
    fn given_changed_and_deleted_items_when_asking_then_question_counts_each() {
        let changed = item(
            Some(1),
            "a",
            BackupRestoreState::Changed {
                current: bookmark(1, "x"),
                restored: bookmark(1, "y"),
                fields: vec!["url"],
            },
        );
        let deleted = item(
            Some(2),
            "b",
            BackupRestoreState::Deleted {
                restored: bookmark(2, "z"),
            },
        );

        let text = question(&[&changed, &deleted]);

        assert!(text.starts_with("Restore 1 changed and 1 deleted bookmarks?"));
    }

    #[test]
    fn given_change_only_in_opener_when_rendering_patch_then_names_the_field_instead_of_an_empty_diff(
    ) {
        let current = bookmark(3, "same");
        let mut restored = bookmark(3, "same");
        restored.opener = Some("vim {}".to_string());
        let changed = item(
            Some(3),
            "a",
            BackupRestoreState::Changed {
                current,
                restored,
                fields: vec!["opener"],
            },
        );

        assert_eq!(item_patch(&changed, false), "#3: only opener differ\n");
    }

    #[test]
    fn given_long_title_when_truncating_then_ellipsis_keeps_the_width() {
        assert_eq!(truncate("abcdefgh", 5), "abcd…");
        assert_eq!(truncate("abc", 5), "abc");
    }
}
