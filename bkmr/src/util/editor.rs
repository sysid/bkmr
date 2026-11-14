// src/util/editor.rs
//! Starting the user's editor, the way git does.

use std::path::Path;
use std::process::Command;

/// `$EDITOR`, or `vi`, which every Unix system has
pub fn configured_editor() -> String {
    std::env::var("EDITOR")
        .ok()
        .filter(|editor| !editor.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string())
}

/// Run `editor` on `path` and wait for it. The editor string goes through `sh` like git's
/// core.editor, so that `EDITOR="code --wait"` and quoted program paths work. A failed start
/// and a non-zero exit are errors.
pub fn edit_file(editor: &str, path: &Path) -> Result<(), String> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$@\"", editor))
        .arg(editor) // $0, shown in the shell's own error messages
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to start editor '{}': {}", editor, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Editor '{}' exited with {}", editor, status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// An editor stub that writes the arguments it received into the file it was given last
    fn recording_editor(dir: &std::path::Path) -> std::path::PathBuf {
        let script = dir.join("record args.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nfor last; do :; done\nprintf '%s|' \"$@\" > \"$last\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[test]
    fn given_editor_with_arguments_when_edit_file_then_runs_it_with_them() {
        // Arrange: EDITOR="code --wait", here with a script whose path has a space
        let dir = tempfile::tempdir().unwrap();
        let editor = format!("'{}' --wait", recording_editor(dir.path()).display());
        let file = dir.path().join("my note.md");
        std::fs::write(&file, "").unwrap();

        // Act
        edit_file(&editor, &file).unwrap();

        // Assert
        let received = std::fs::read_to_string(&file).unwrap();
        assert_eq!(received, format!("--wait|{}|", file.display()));
    }

    #[test]
    fn given_editor_that_fails_when_edit_file_then_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.md");

        let result = edit_file("false", &file);

        let err = result.expect_err("a failing editor is an error");
        assert!(err.contains("false"), "got: {err}");
    }
}
