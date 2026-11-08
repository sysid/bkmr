// bkmr/src/cli/skill.rs
//! The bkmr-memory agent skill, embedded so every install channel ships a copy that matches the
//! binary's CLI flags.

use crate::cli::error::{CliError, CliResult};
use std::fs;
use std::path::{Path, PathBuf};

/// Directory name of the skill; the Agent Skills spec requires it to equal the `name` field.
pub const SKILL_NAME: &str = "bkmr-memory";

/// (path relative to the skill directory, content).
// ponytail: explicit list instead of an `include_dir` dependency; tests/test_skill.rs fails when
// a file in skills/bkmr-memory is missing here.
pub const SKILL_FILES: &[(&str, &str)] = &[
    (
        "SKILL.md",
        include_str!("../../skills/bkmr-memory/SKILL.md"),
    ),
    (
        "scripts/due-report.py",
        include_str!("../../skills/bkmr-memory/scripts/due-report.py"),
    ),
];

/// Writes the embedded skill to `dir/bkmr-memory` and returns that path.
///
/// An existing `dir/bkmr-memory` is an error unless `force` is set. With `force` it is removed
/// first, so files dropped in a newer version do not linger. A symlink there is removed as a link:
/// users often link the skill to a repo checkout, and that checkout must never be overwritten.
pub fn install(dir: &Path, force: bool) -> CliResult<PathBuf> {
    let target = dir.join(SKILL_NAME);
    let io_error = |path: &Path, e: std::io::Error| {
        CliError::CommandFailed(format!("{}: {e}", path.display()))
    };

    // symlink_metadata sees the link itself, also when it dangles.
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if !force {
            return Err(CliError::AlreadyExists(format!(
                "{} (use --force to replace it)",
                target.display()
            )));
        }
        let removed = if meta.file_type().is_symlink() {
            // Windows needs remove_dir for a directory symlink; neither follows the link.
            fs::remove_file(&target).or_else(|_| fs::remove_dir(&target))
        } else if meta.is_dir() {
            fs::remove_dir_all(&target)
        } else {
            fs::remove_file(&target)
        };
        removed.map_err(|e| io_error(&target, e))?;
    }

    for (path, content) in SKILL_FILES {
        let file = target.join(path);
        let parent = file.parent().expect("skill file has a parent directory");
        fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        fs::write(&file, content).map_err(|e| io_error(&file, e))?;
        if path.starts_with("scripts/") {
            make_executable(&file).map_err(|e| io_error(&file, e))?;
        }
    }
    Ok(target)
}

#[cfg(unix)]
fn make_executable(file: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(file, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_file: &Path) -> std::io::Result<()> {
    Ok(())
}
