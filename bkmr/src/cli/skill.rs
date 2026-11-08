// bkmr/src/cli/skill.rs
//! The bkmr-memory agent skill, embedded so every install channel ships a copy that matches the
//! binary's CLI flags.

use crate::cli::error::{CliError, CliResult};
use crate::util::path::expand_path;
use std::fs;
use std::io::ErrorKind;
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
/// `dir` may contain `~` or `$VAR`: agents often call bkmr without a shell.
///
/// An existing `dir/bkmr-memory` is an error unless `force` is set. With `force` it is replaced as a
/// whole, so files dropped in a newer version do not linger. A symlink there is replaced as a link
/// and its target is left alone. A real directory is deleted, also when `dir` itself resolves into
/// a repo checkout through a symlink.
pub fn install(dir: &Path, force: bool) -> CliResult<PathBuf> {
    // Non-UTF-8 paths cannot hold `~` or `$VAR` worth expanding; use them as given.
    let dir = match dir.to_str() {
        Some(s) => PathBuf::from(expand_path(s)),
        None => dir.to_path_buf(),
    };
    install_files(&dir, force, SKILL_FILES)
}

/// The bundle is written to a staging directory and renamed into place, so a failed write never
/// leaves a partial skill behind and `--force` keeps the old copy until the new one is complete.
fn install_files(dir: &Path, force: bool, files: &[(&str, &str)]) -> CliResult<PathBuf> {
    let target = dir.join(SKILL_NAME);
    let staging = dir.join(format!(".{SKILL_NAME}.new"));
    let backup = dir.join(format!(".{SKILL_NAME}.old"));

    // symlink_metadata sees the link itself, also when it dangles.
    let exists = match fs::symlink_metadata(&target) {
        Ok(_) => true,
        Err(e) if e.kind() == ErrorKind::NotFound => false,
        Err(e) => return Err(io_error(&target, e)),
    };
    if exists && !force {
        return Err(CliError::AlreadyExists(format!(
            "{} (use --force to replace it)",
            target.display()
        )));
    }

    fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    // Leftovers of an interrupted install; both names belong to bkmr.
    remove_entry(&staging)?;
    remove_entry(&backup)?;

    if let Err(e) = write_bundle(&staging, files) {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    // rename moves a symlink as a link, so the old entry is set aside whatever it is.
    if exists {
        if let Err(e) = fs::rename(&target, &backup) {
            let _ = fs::remove_dir_all(&staging);
            return Err(io_error(&target, e));
        }
    }
    if let Err(e) = fs::rename(&staging, &target) {
        if exists {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(io_error(&target, e));
    }
    // ponytail: the new skill is in place, so a failed cleanup does not fail the install; the next
    // install removes the leftover backup.
    let _ = remove_entry(&backup);
    Ok(target)
}

fn write_bundle(root: &Path, files: &[(&str, &str)]) -> CliResult<()> {
    for (path, content) in files {
        let file = root.join(path);
        let parent = file.parent().expect("skill file has a parent directory");
        fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        fs::write(&file, content).map_err(|e| io_error(&file, e))?;
        if path.starts_with("scripts/") {
            make_executable(&file).map_err(|e| io_error(&file, e))?;
        }
    }
    Ok(())
}

/// Removes `path` if present, without following a symlink.
fn remove_entry(path: &Path) -> CliResult<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io_error(path, e)),
    };
    let removed = if meta.file_type().is_symlink() {
        // Windows needs remove_dir for a directory symlink; neither follows the link.
        fs::remove_file(path).or_else(|_| fs::remove_dir(path))
    } else if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    removed.map_err(|e| io_error(path, e))
}

fn io_error(path: &Path, e: std::io::Error) -> CliError {
    CliError::CommandFailed(format!("{}: {e}", path.display()))
}

/// Adds execute bits where the file has read bits, so the mode follows the user's umask.
#[cfg(unix)]
fn make_executable(file: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(file)?.permissions();
    let mode = permissions.mode();
    permissions.set_mode(mode | (mode & 0o444) >> 2);
    fs::set_permissions(file, permissions)
}

#[cfg(not(unix))]
fn make_executable(_file: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // The second entry needs SKILL.md to be a directory, so writing the bundle fails after the
    // first file is already written.
    const BUNDLE_FAILING_MIDWAY: &[(&str, &str)] =
        &[("SKILL.md", "new version"), ("SKILL.md/boom", "unwritable")];

    #[test]
    fn given_installed_skill_when_forced_install_fails_midway_then_old_copy_is_kept() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join(SKILL_NAME);
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "old version").unwrap();

        let result = install_files(dir.path(), true, BUNDLE_FAILING_MIDWAY);

        assert!(matches!(result, Err(CliError::CommandFailed(_))));
        assert_eq!(
            fs::read_to_string(skill.join("SKILL.md")).unwrap(),
            "old version"
        );
    }

    #[test]
    fn given_no_skill_when_install_fails_midway_then_nothing_is_left_behind() {
        let dir = TempDir::new().unwrap();

        let result = install_files(dir.path(), false, BUNDLE_FAILING_MIDWAY);

        assert!(result.is_err());
        let leftovers: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }

    #[test]
    fn given_failed_install_when_retrying_then_installs_without_force() {
        let dir = TempDir::new().unwrap();
        let _ = install_files(dir.path(), false, BUNDLE_FAILING_MIDWAY);

        let installed = install_files(dir.path(), false, SKILL_FILES).unwrap();

        assert!(installed.join("SKILL.md").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn given_unsearchable_dir_when_installing_then_fails_on_the_existence_check() {
        use std::os::unix::fs::PermissionsExt;
        let base = TempDir::new().unwrap();
        let dir = base.path().join("locked");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();

        let result = install_files(&dir, false, SKILL_FILES);

        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        let Err(CliError::CommandFailed(msg)) = result else {
            panic!("expected CommandFailed, got {result:?}");
        };
        assert!(
            msg.starts_with(&format!("{}:", dir.join(SKILL_NAME).display())),
            "error should name the checked path: {msg}"
        );
    }
}
