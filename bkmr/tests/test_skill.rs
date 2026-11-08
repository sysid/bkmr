//! The bkmr-memory skill bundle: Agent Skills spec conformance and `bkmr skill install`.
//! Spec: https://agentskills.io/specification

use assert_cmd::cargo::cargo_bin_cmd;
use bkmr::cli::skill::{SKILL_FILES, SKILL_NAME};
use predicates::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn skill_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("skills")
        .join(SKILL_NAME)
}

fn skill_md() -> String {
    fs::read_to_string(skill_dir().join("SKILL.md")).expect("SKILL.md exists")
}

fn frontmatter() -> BTreeMap<String, serde_yaml::Value> {
    let text = skill_md();
    let rest = text
        .strip_prefix("---\n")
        .expect("SKILL.md starts with a YAML frontmatter fence");
    let end = rest
        .find("\n---\n")
        .expect("frontmatter has a closing fence");
    serde_yaml::from_str(&rest[..end]).expect("frontmatter is valid YAML")
}

fn field(name: &str) -> Option<String> {
    frontmatter()
        .get(name)
        .map(|v| v.as_str().expect("field is a string").to_string())
}

// --- Spec conformance -------------------------------------------------------------------------

#[test]
fn given_skill_frontmatter_when_checking_keys_then_only_spec_fields_are_used() {
    let allowed = [
        "name",
        "description",
        "license",
        "compatibility",
        "metadata",
        "allowed-tools",
    ];
    for key in frontmatter().keys() {
        assert!(
            allowed.contains(&key.as_str()),
            "non-spec frontmatter key: {key}"
        );
    }
}

#[test]
fn given_skill_name_when_checking_then_it_follows_spec_naming_and_matches_directory() {
    let name = field("name").expect("name is required");
    let valid = regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap();

    assert!(name.len() <= 64, "name longer than 64 characters");
    assert!(
        valid.is_match(&name),
        "name must be lowercase alphanumerics and single inner hyphens: {name}"
    );
    assert_eq!(name, SKILL_NAME, "name must match the skill directory name");
    assert_eq!(
        skill_dir().file_name().unwrap().to_str().unwrap(),
        SKILL_NAME
    );
}

#[test]
fn given_skill_description_when_checking_then_it_is_between_1_and_1024_characters() {
    let description = field("description").expect("description is required");
    let length = description.trim().chars().count();
    assert!((1..=1024).contains(&length), "description length {length}");
}

#[test]
fn given_skill_compatibility_when_checking_then_it_names_bkmr_within_500_characters() {
    let compatibility = field("compatibility").expect("compatibility tells agents bkmr is needed");
    assert!(compatibility.chars().count() <= 500);
    assert!(compatibility.contains("bkmr"));
}

#[test]
fn given_skill_license_when_checking_then_it_matches_the_project_license() {
    assert_eq!(field("license").as_deref(), Some("BSD-3-Clause"));
}

#[test]
fn given_skill_body_when_scanning_file_references_then_each_exists_in_the_bundle() {
    // ponytail: only paths under the bundle's own top-level dirs count as references; example
    // paths inside memory samples (`config/db.yml`) are not files of the skill. Add a prefix here
    // when the bundle grows a new top-level directory.
    let reference = regex::Regex::new(r"\b((?:scripts|references|assets|evals)/[\w./-]+)").unwrap();
    let body = skill_md();
    let missing: Vec<_> = reference
        .captures_iter(&body)
        .map(|c| c[1].trim_end_matches(['.', ')']).to_string())
        .filter(|path| !skill_dir().join(path).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "SKILL.md references files that are not shipped: {missing:?}"
    );
}

#[test]
fn given_skill_directory_when_comparing_with_embedded_files_then_both_list_the_same_files() {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if name == "__pycache__" || name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                out.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut on_disk = BTreeSet::new();
    walk(&skill_dir(), &skill_dir(), &mut on_disk);
    let embedded: BTreeSet<String> = SKILL_FILES.iter().map(|f| f.0.to_string()).collect();

    assert_eq!(
        embedded, on_disk,
        "SKILL_FILES in src/cli/skill.rs is out of date"
    );
}

// --- bkmr skill install -----------------------------------------------------------------------

#[test]
fn given_empty_directory_when_installing_then_writes_the_embedded_bundle() {
    let target = TempDir::new().unwrap();

    cargo_bin_cmd!("bkmr")
        .args(["skill", "install"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(SKILL_NAME));

    for (path, content) in SKILL_FILES {
        let installed = target.path().join(SKILL_NAME).join(path);
        assert_eq!(
            fs::read_to_string(&installed).unwrap(),
            *content,
            "{path} differs from the embedded copy"
        );
    }
}

#[cfg(unix)]
#[test]
fn given_bundle_with_scripts_when_installing_then_scripts_are_executable() {
    use std::os::unix::fs::PermissionsExt;
    let target = TempDir::new().unwrap();

    cargo_bin_cmd!("bkmr")
        .args(["skill", "install"])
        .arg(target.path())
        .assert()
        .success();

    let script = target.path().join(SKILL_NAME).join("scripts/due-report.py");
    let mode = fs::metadata(script).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "due-report.py is not executable");
}

#[test]
fn given_missing_target_directory_when_installing_then_creates_it() {
    let base = TempDir::new().unwrap();
    let target = base.path().join("not/yet/there");

    cargo_bin_cmd!("bkmr")
        .args(["skill", "install"])
        .arg(&target)
        .assert()
        .success();

    assert!(target.join(SKILL_NAME).join("SKILL.md").is_file());
}

#[test]
fn given_no_database_when_installing_then_still_succeeds() {
    let target = TempDir::new().unwrap();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", target.path().join("does-not-exist.db"))
        .args(["skill", "install"])
        .arg(target.path())
        .assert()
        .success();
}

#[test]
fn given_installed_skill_when_installing_without_force_then_exits_with_duplicate_code_and_keeps_it()
{
    let target = TempDir::new().unwrap();
    let skill = target.path().join(SKILL_NAME);
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "user's own copy").unwrap();

    cargo_bin_cmd!("bkmr")
        .args(["skill", "install"])
        .arg(target.path())
        .assert()
        .code(65)
        .stderr(predicate::str::contains("--force"));

    assert_eq!(
        fs::read_to_string(skill.join("SKILL.md")).unwrap(),
        "user's own copy"
    );
}

#[test]
fn given_installed_skill_with_stale_file_when_installing_with_force_then_replaces_whole_bundle() {
    let target = TempDir::new().unwrap();
    let skill = target.path().join(SKILL_NAME);
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "old version").unwrap();
    fs::write(skill.join("dropped-in-new-version.md"), "stale").unwrap();

    cargo_bin_cmd!("bkmr")
        .args(["skill", "install", "--force"])
        .arg(target.path())
        .assert()
        .success();

    assert_ne!(
        fs::read_to_string(skill.join("SKILL.md")).unwrap(),
        "old version"
    );
    assert!(!skill.join("dropped-in-new-version.md").exists());
}

#[cfg(unix)]
#[test]
fn given_skill_path_is_symlink_when_installing_with_force_then_replaces_link_and_keeps_link_target()
{
    let base = TempDir::new().unwrap();
    let checkout = base.path().join("checkout/bkmr-memory");
    fs::create_dir_all(&checkout).unwrap();
    fs::write(checkout.join("SKILL.md"), "repo copy").unwrap();
    let target = base.path().join("skills");
    fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&checkout, target.join(SKILL_NAME)).unwrap();

    cargo_bin_cmd!("bkmr")
        .args(["skill", "install", "--force"])
        .arg(&target)
        .assert()
        .success();

    let installed = target.join(SKILL_NAME);
    assert!(
        !installed.is_symlink(),
        "link should be replaced by a real directory"
    );
    assert!(installed.join("SKILL.md").is_file());
    assert_eq!(
        fs::read_to_string(checkout.join("SKILL.md")).unwrap(),
        "repo copy",
        "the link target must never be touched"
    );
}
