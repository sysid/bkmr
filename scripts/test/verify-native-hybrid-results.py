#!/usr/bin/env python3
"""Data-only audit of native Test workflow artifacts; never launches a process.

The existing Actions job/step owns execution and finite host retirement. This
helper records files and validates exact native observations, never constructs
an Output, substitutes a provider, downloads a model, or converts a failure to
a semantic pass. Complete raw files remain when a bounded parse is refused.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tarfile
import tempfile
from datetime import datetime, timezone

FILE_LIMIT = 512 * 1024 * 1024
PARSE_LIMIT = 16 * 1024 * 1024
MATERIAL_LIMIT = 1024 * 1024 * 1024
OLD_ARCHIVE_SHA = "a7350a3372263fc7e89f8b80f8d4f7966a19aa5c08a50d0a89b5c9c67466b7f2"
OLD_MANIFEST_SHA = "1b509fb7e27e18252a618a3ba15c613e7ae0de26b4dc6bc95a122e7b6759c289"
OLD_LOCK_SHA = "b40abff62a394ecf4f41a3c6f461b36c42a24d1077b1c7346167fa9116dd9726"
EXPECTED_SOURCE = {
    "bkmr/src/application/error.rs": "2aafe36786f2ef493b1197aa37d9b9859cbf18e27fe5df7a24960d0f592591a2",
    "bkmr/src/application/services/bookmark_service_impl.rs": "dcb4187dac242b9f0982d6f73460513266d3b398e2e9b0af5a16cfa566ae9f4d",
    "bkmr/src/cli/args.rs": "4eb80a46fb8b86e6b88dad3abb561b258bccae2e499f82da643052a10a400d67",
    "bkmr/src/cli/hsearch_handler.rs": "7be98e7d8f9a9bb4638d81198c08290318d9ce3fd0f41fdc02a3eaf45465305e",
    "bkmr/src/domain/error.rs": "56a70295e65da0d4a50024f034471aef5def6d443362a227afaff6c82c0fdc60",
    "bkmr/src/domain/search.rs": "5e38f05331b8b5acec83216f31aca1eb97d3704ecfbc3fd8b47d5bc70df980da",
    "bkmr/src/infrastructure/di/service_container.rs": "53a0946baad01a607f69a4e3c418c9fcc2c3b83ce3235768f69bfc639de18ec3",
    "bkmr/src/infrastructure/repositories/sqlite/connection.rs": "3eabb3842789062089583dc7d62fa1ebaba2946d7ced034c5ea22f79bf3681b5",
    "bkmr/src/infrastructure/repositories/sqlite/error.rs": "34b7331b080d044331dde8a55d4e5a449db91148cefa7cc769192a671b7add4e",
    "bkmr/src/infrastructure/repositories/sqlite/vector_repository.rs": "8e87dac85d6e2200aeb8cb94b7f4a8f6a0b2022c839e58582cc9176436326613",
    "bkmr/src/main.rs": "5abe532bcfa9b2811a8d01ec79da776a98952d356096422aa59fdad7f00622ea",
    "bkmr/tests/test_hybrid_literal_fts.rs": "dc89d0027ef854241e4c6b94b4e3da3c1c1cd8853f37cc9ceb546458c43ff40d",
    "bkmr/Cargo.toml": "dc0500b057de4ceb9a70209ed739bd1477275c9f2f288d7320f9d6c69eba2ecc",
    "bkmr/Cargo.lock": "ef84de82354e697d36a903381a8df4e644d3af737da081e364eda2d0ec05fcdd",
}
HYBRID = [
    "given_punctuation_when_literal_fts_then_actual_sqlite_matches_without_changing_subject",
    "given_raw_boolean_grammar_when_opt_in_changes_then_actual_sqlite_keeps_default_grammar",
    "given_native_exact_cli_when_literal_subject_then_real_id_tags_hydration_and_rrf_are_preserved",
    "given_actual_cli_when_raw_or_literal_then_defaults_and_advertised_opt_in_remain_distinct",
    "given_empty_native_results_when_json_requested_then_success_is_an_actual_empty_array",
    "given_whitespace_literal_when_native_tag_prefilter_is_empty_then_validation_still_refuses",
    "given_actual_old_native_binary_when_literal_flag_requested_then_unsupported_is_explicit_without_downgrade",
    "given_genuine_native_model_when_literal_hybrid_then_original_input_and_actual_vector_contribution_survive",
    "given_real_empty_vector_table_when_checked_then_absence_is_healthy_and_restart_stable",
    "given_actual_missing_vector_table_when_checked_then_native_repository_error_survives_recovery",
    "given_genuine_native_model_when_vector_presence_query_fails_then_hybrid_preserves_error_and_healthy_fallback",
    "given_corrupt_native_database_when_initializing_vectors_then_schema_error_precedes_create",
    "given_real_missing_vector_schema_when_initialized_then_create_and_existing_restart_are_healthy",
    "given_real_missing_table_when_native_cause_crosses_context_then_same_sqlite_failure_survives",
    "given_corrupt_native_database_when_production_cli_initializes_then_wal_failure_and_exit_are_preserved",
]
UNIT = [
    ("lib", "infrastructure::repositories::sqlite::vector_repository::native_readonly_tests::given_actual_readonly_sqlite_connection_when_missing_table_is_created_then_original_cause_is_retained"),
    ("lib", "infrastructure::di::service_container::native_cause_tests::given_corrupt_owned_sqlite_file_when_native_repository_is_created_then_original_wal_cause_reaches_application"),
    ("bin", "tests::given_actual_native_vector_failure_when_main_wraps_then_display_and_original_source_are_preserved"),
    ("lib", "infrastructure::repositories::sqlite::connection::backup_observation_tests::given_genuinely_absent_bookmark_relation_when_backup_preflight_then_absence_is_healthy_and_native_creation_reopens"),
    ("lib", "infrastructure::repositories::sqlite::connection::backup_observation_tests::given_empty_and_populated_native_bookmarks_when_backup_preflight_then_exact_count_controls_backup"),
    ("lib", "infrastructure::repositories::sqlite::connection::backup_observation_tests::given_actual_sqlite_schema_lock_when_backup_preflight_then_original_diesel_failure_is_unavailable_not_empty"),
    ("lib", "infrastructure::repositories::sqlite::connection::backup_observation_tests::given_real_broken_bookmark_view_when_backup_preflight_then_failed_count_is_not_absence_and_repair_reopens"),
    ("lib", "infrastructure::repositories::sqlite::connection::backup_observation_tests::given_working_case_and_view_bookmark_relations_when_backup_preflight_then_existing_count_compatibility_survives"),
    ("lib", "infrastructure::repositories::sqlite::connection::backup_observation_tests::given_pending_native_migration_and_real_failed_count_when_migrating_then_failure_precedes_backup_and_mutation"),
]
IGNORED = {HYBRID[6]: "before", HYBRID[7]: "model-hybrid", HYBRID[10]: "model-error"}
CASES = [
    {"id": "%02d" % (i + 1), "role": "hybrid", "name": name,
     "ignored": name in IGNORED, "group": IGNORED.get(name, "default")}
    for i, name in enumerate(HYBRID)
] + [
    {"id": "%02d" % (i + 16), "role": role, "name": name,
     "ignored": False, "group": "default"}
    for i, (role, name) in enumerate(UNIT)
]


def now():
    return datetime.now(timezone.utc).isoformat()


def dump(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def digest(path, limit=FILE_LIMIT):
    path = Path(path)
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise ValueError("artifact is not an ordinary regular file: %s" % path)
    result = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        while True:
            chunk = stream.read(65536)
            if not chunk:
                break
            size += len(chunk)
            if size > limit:
                raise ValueError("artifact observation capacity exceeded: %s" % path)
            result.update(chunk)
    after = path.lstat()
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
        raise ValueError("artifact changed during observation: %s" % path)
    return {"path": str(path), "sha256": result.hexdigest(), "bytes": size,
            "mode": oct(stat.S_IMODE(after.st_mode))}


def text(path):
    if Path(path).stat().st_size > PARSE_LIMIT:
        raise ValueError("parse capacity exceeded; full raw file retained: %s" % path)
    return Path(path).read_text(encoding="utf-8", errors="strict")


def host_write(destination, values):
    lines = []
    for key, value in values.items():
        value = str(value)
        if "\n" in value or "\r" in value:
            raise ValueError("unsupported newline in actual hosted coordinate")
        lines.append(key + "=" + value + "\n")
    with open(os.environ[destination], "a", encoding="utf-8") as output:
        output.write("".join(lines))


def env_write(values):
    host_write("GITHUB_ENV", values)


def output_write(values):
    host_write("GITHUB_OUTPUT", values)


def source_check(workspace):
    observed = []
    for relative, expected in EXPECTED_SOURCE.items():
        item = digest(workspace / relative)
        if item["sha256"] != expected:
            raise ValueError("candidate Source mismatch: " + relative)
        observed.append(item)
    with (workspace / "TESTING.md").open("rb") as stream:
        prefix = stream.read(24973)
    if hashlib.sha256(prefix).hexdigest() != "8ef5858cbb0c5ce54e38b4a347e09691f1445e2d23c0eab8dec5e1815ee8aa96":
        raise ValueError("corrected be96 TESTING pairing was not preserved")
    return observed


def identity(path):
    value = Path(path).lstat()
    if not stat.S_ISDIR(value.st_mode):
        raise ValueError("owned Run path is not a physical directory")
    return [value.st_dev, value.st_ino]


def prepare():
    workspace = Path(os.environ["GITHUB_WORKSPACE"]).resolve(strict=True)
    observed = source_check(workspace)
    base = workspace / "ProjectCentral/now/tmp"
    base.mkdir(parents=True, exist_ok=True)
    # A test material location, not a semantic World/Project allocation.
    if base.resolve(strict=True) != base:
        raise ValueError("test Run parent must not redirect outside the actual checkout")
    root = Path(tempfile.mkdtemp(prefix="native-hybrid-", dir=str(base)))
    record = {"utc": now(), "workspace": str(workspace), "base": str(base),
              "base_identity": identity(base), "root": str(root),
              "root_identity": identity(root), "evidence": None,
              "evidence_identity": None, "fixture": None,
              "fixture_identity": None, "prepared": False, "phase": "allocated"}
    # Publish only actual observed custody, before any fixture effects. The same
    # existing owner record is carried by the host if its first disk write fails.
    seed = json.dumps(record, separators=(",", ":"))
    print("native Run custody: " + seed, flush=True)
    os.environ["BKMR_NATIVE_QUALIFICATION_ROOT"] = str(root)
    os.environ["BKMR_NATIVE_ROOT_CUSTODY"] = seed
    try:
        output_write({"owned_root": root, "owned_custody": seed})
        dump(root / "ownership.json", record)
        evidence = root / "evidence"
        evidence.mkdir()
        record.update(evidence=str(evidence), evidence_identity=identity(evidence),
                      phase="evidence-created")
        seed = json.dumps(record, separators=(",", ":"))
        os.environ["BKMR_NATIVE_ROOT_CUSTODY"] = seed
        output_write({"evidence_path": evidence, "owned_custody": seed})
        dump(root / "ownership.json", record)
        # Evidence custody is published before fixture creation; fallback to the
        # admitted root cannot sweep a later unbounded build tree.
        fixture = root / "fixture"
        fixture.mkdir()
        record.update(fixture=str(fixture), fixture_identity=identity(fixture),
                      phase="fixture-created")
        seed = json.dumps(record, separators=(",", ":"))
        os.environ["BKMR_NATIVE_ROOT_CUSTODY"] = seed
        output_write({"owned_custody": seed})
        dump(root / "ownership.json", record)
        for member in ["home", "xdg-config", "xdg-cache", "model-cache", "warm"]:
            (fixture / member).mkdir()
        dump(evidence / "candidate-source-before.json", observed)
        dump(evidence / "selection.json", CASES)
        dump(evidence / "host-context.json", {key: os.environ.get(key) for key in
             ["GITHUB_REPOSITORY", "GITHUB_SHA", "GITHUB_REF", "GITHUB_RUN_ID",
              "GITHUB_RUN_ATTEMPT", "RUNNER_OS", "RUNNER_ARCH"]})
        for group in ["default", "before", "model-hybrid", "model-error"]:
            (evidence / (group + ".tsv")).write_text("".join(
                "\t".join([c["id"], c["role"], "1" if c["ignored"] else "0", c["name"]]) + "\n"
                for c in CASES if c["group"] == group))
        (fixture / "warm/config.toml").write_text('[embeddings]\nmodel = "AllMiniLML6V2"\n')
        original_home = Path(os.environ["HOME"])
        env_write({"BKMR_NATIVE_QUALIFICATION_ROOT": root, "BKMR_EVIDENCE": evidence,
                   "BKMR_NATIVE_TEST_ROOT": fixture, "HOME": fixture / "home",
                   "XDG_CONFIG_HOME": fixture / "xdg-config", "XDG_CACHE_HOME": fixture / "xdg-cache",
                   "FASTEMBED_CACHE_DIR": fixture / "model-cache",
                   "BKMR_LITERAL_FTS_MODEL": "AllMiniLML6V2", "NO_COLOR": "1",
                   "CARGO_TARGET_DIR": workspace / "bkmr/target",
                   "CARGO_HOME": os.environ.get("CARGO_HOME", original_home / ".cargo"),
                   "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", original_home / ".rustup")})
        record.update(prepared=True, phase="prepared")
        dump(root / "ownership.json", record)
    except (OSError, ValueError, KeyError) as error:
        failure = {"utc": now(), "phase": record["phase"], "prepared": False,
                   "actual_type": type(error).__name__,
                   "actual_errno": getattr(error, "errno", None), "message": str(error),
                   "observed_custody": record, "fixture_removed": False}
        print("native preparation failed: " + json.dumps(failure), file=sys.stderr, flush=True)
        try:
            if identity(root) != record["root_identity"] or identity(base) != record["base_identity"]:
                raise ValueError("partial preparation custody unavailable; no redirected write")
            dump(root / "prepare-error.json", failure)
        except (OSError, ValueError) as retention_error:
            print("native preparation error retention failed: " + str(retention_error),
                  file=sys.stderr, flush=True)
        raise


def owner(allow_partial=False):
    root = Path(os.environ["BKMR_NATIVE_QUALIFICATION_ROOT"])
    if allow_partial and os.environ.get("BKMR_NATIVE_ROOT_CUSTODY"):
        # SAME actual checkpoint record carried by the host, not a guessed
        # locator or a replacement authority. Partial writes cannot erase it.
        record = json.loads(os.environ["BKMR_NATIVE_ROOT_CUSTODY"])
    else:
        record = json.loads(text(root / "ownership.json"))
    workspace = Path(record["workspace"])
    base = Path(record["base"])
    if (not root.is_absolute() or str(root) != record["root"]
            or base != workspace / "ProjectCentral/now/tmp" or root.parent != base
            or identity(base) != record["base_identity"]
            or identity(root) != record["root_identity"]):
        raise ValueError("owned native Run root affiliation was lost")
    evidence = Path(record["evidence"]) if record["evidence"] is not None else root
    if evidence != root:
        if evidence != root / "evidence" or identity(evidence) != record["evidence_identity"]:
            raise ValueError("owned native evidence affiliation was lost")
    fixture = Path(record["fixture"]) if record["fixture"] is not None else None
    if fixture is not None:
        if fixture != root / "fixture" or identity(fixture) != record["fixture_identity"]:
            raise ValueError("owned native fixture affiliation was lost")
    if not allow_partial and (not record["prepared"] or fixture is None or evidence == root):
        raise ValueError("native Run preparation incomplete; no subject execution admitted")
    return evidence, fixture, workspace


def old_source():
    evidence, fixture, _ = owner()
    archive = fixture / "bkmr-7.6.7.tar.gz"
    receipt = digest(archive, 1024 * 1024)
    if receipt["sha256"] != OLD_ARCHIVE_SHA or receipt["bytes"] != 424282:
        raise ValueError("unmodified native7.6.7 Source archive mismatch")
    destination = fixture / "old-source"
    destination.mkdir()
    members = []
    total = 0
    with tarfile.open(archive, "r:gz") as source:
        for member in source:
            parts = PurePosixPath(member.name).parts
            if (not parts or parts[0] != "bkmr-7.6.7" or ".." in parts
                    or member.name.startswith("/") or not (member.isdir() or member.isfile())):
                raise ValueError("unsupported pinned Source archive member")
            if len(members) >= 10000 or member.size > PARSE_LIMIT:
                raise ValueError("pinned Source archive observation capacity exceeded")
            total += member.size
            if total > 64 * 1024 * 1024:
                raise ValueError("pinned Source archive total capacity exceeded")
            target = destination.joinpath(*parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as input_file, target.open("xb") as output:
                    shutil.copyfileobj(input_file, output, 65536)
                target.chmod(member.mode & 0o777)
                members.append(digest(target))
    project = destination / "bkmr-7.6.7"
    manifest = digest(project / "bkmr/Cargo.toml")
    lock = digest(project / "bkmr/Cargo.lock")
    if manifest["sha256"] != OLD_MANIFEST_SHA or lock["sha256"] != OLD_LOCK_SHA:
        raise ValueError("unmodified native7.6.7 manifest/lock mismatch")
    dump(evidence / "old-source.json", {"archive": receipt, "manifest": manifest,
         "lock": lock, "members": members, "legacy_tag": "a6ca05ef4e20baa3c59d96653e8bdf734a27fca1",
         "source_relation": "pinned published7.6.7 sdist; separate API/lock from maintained7.6.11"})
    env_write({"BKMR_OLD_PROJECT": project})


def compiler(phase):
    evidence, fixture, workspace = owner()
    paths = [evidence / (phase + "-compiler.jsonl")]
    if phase == "candidate":
        paths.append(evidence / "candidate-cli-compiler.jsonl")
    found = {}
    records = []
    manifest = workspace / "bkmr/Cargo.toml" if phase == "candidate" else Path(os.environ["BKMR_OLD_PROJECT"]) / "bkmr/Cargo.toml"
    for path in paths:
        for line in text(path).splitlines():
            item = json.loads(line)
            if item.get("reason") != "compiler-artifact" or not item.get("executable"):
                continue
            if Path(item["manifest_path"]).resolve(strict=True) != manifest.resolve(strict=True):
                continue
            target = item["target"]
            kinds = target["kind"]
            role = None
            if phase == "old":
                if target["name"] == "bkmr" and kinds == ["bin"] and not item["profile"]["test"]:
                    role = "old"
            elif item["profile"]["test"]:
                if target["name"] == "test_hybrid_literal_fts" and kinds == ["test"]:
                    role = "hybrid"
                elif target["name"] == "bkmr" and kinds == ["lib"]:
                    role = "lib"
                elif target["name"] == "bkmr" and kinds == ["bin"]:
                    role = "bin"
            elif target["name"] == "bkmr" and kinds == ["bin"]:
                role = "cli"
            if role:
                path = Path(item["executable"])
                permitted = fixture / "old-target" if phase == "old" else workspace / "bkmr/target"
                path.resolve(strict=True).relative_to(permitted.resolve(strict=True))
                observed = digest(path)
                if role in found and found[role]["sha256"] != observed["sha256"]:
                    raise ValueError("multiple incompatible compiler-produced native artifacts")
                found[role] = observed
                records.append(item)
    required = {"old"} if phase == "old" else {"hybrid", "lib", "bin", "cli"}
    if set(found) != required:
        raise ValueError("native compiler executable roles missing: " + str(required - set(found)))
    dump(evidence / (phase + "-executables.json"), {"artifacts": found, "compiler_records": records})
    if phase == "old":
        env_write({"BKMR_LITERAL_FTS_BEFORE_BIN": found["old"]["path"],
                   "BKMR_LITERAL_FTS_BEFORE_SHA256": found["old"]["sha256"]})
    else:
        env_write({"HYBRID_TEST_BIN": found["hybrid"]["path"], "LIB_TEST_BIN": found["lib"]["path"],
                   "BIN_TEST_BIN": found["bin"]["path"], "BKMR_NATIVE_BIN": found["cli"]["path"]})


def rosters():
    evidence, _, _ = owner()
    expected = {"hybrid": set(HYBRID), "lib": {name for role, name in UNIT if role == "lib"},
                "bin": {name for role, name in UNIT if role == "bin"}}
    observed = {}
    for role, names in expected.items():
        raw = text(evidence / (role + "-roster.stdout"))
        if text(evidence / (role + "-roster.status")).strip() != "0":
            raise ValueError("compiled native roster command failed: " + role)
        rows = re.findall(r"^(.+): test$", raw, re.M)
        if len(rows) != len(set(rows)) or not names.issubset(set(rows)):
            raise ValueError("compiled native roster missing/duplicate case: " + role)
        if role == "hybrid" and set(rows) != names:
            raise ValueError("integration compiled roster differs from exact15 definitions")
        observed[role] = {"compiled_names": rows, "selected": sorted(names),
                          "stdout": digest(evidence / (role + "-roster.stdout"))}
    dump(evidence / "compiled-rosters.json", observed)


def case_start(case_id):
    evidence, _, _ = owner()
    case = next(c for c in CASES if c["id"] == case_id)
    dump(evidence / ("case-" + case_id + ".started.json"), {"utc": now(), **case})


def case_result(case_id):
    evidence, _, _ = owner()
    case = next(c for c in CASES if c["id"] == case_id)
    prefix = evidence / ("case-" + case_id)
    status = int(text(str(prefix) + ".status").strip())
    stdout = text(str(prefix) + ".stdout")
    passed_line = re.search(r"^test " + re.escape(case["name"]) + r" \.\.\. ok$", stdout, re.M)
    summaries = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured;", stdout)
    passed = status == 0 and bool(passed_line) and summaries == [("ok", "1", "0", "0", "0")]
    dump(str(prefix) + ".result.json", {"utc": now(), **case, "actual_native_exit": status,
         "native_summary": summaries, "exact_named_pass": bool(passed_line), "accepted": passed,
         "stdout": digest(str(prefix) + ".stdout"), "stderr": digest(str(prefix) + ".stderr"),
         "scope": "exact actual dispatch; prerequisite refusals/failures/ignored/0tests are not semantic passes"})
    if not passed:
        raise ValueError("exact native selected case did not pass: " + case["name"])


def copy_artifact(source, destination):
    before = digest(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with Path(source).open("rb") as input_file, destination.open("xb") as output:
        shutil.copyfileobj(input_file, output, 65536)
    observed = digest(destination)
    if before["sha256"] != observed["sha256"]:
        raise ValueError("copied actual artifact disagrees with original bytes")
    return {"original": before, "retained": observed}


def retention_start():
    evidence, _, _ = owner(allow_partial=True)
    if Path(os.environ["BKMR_EVIDENCE"]) != evidence:
        raise ValueError("hosted retention mapping disagrees with actual custody")
    output_write({"retained_path": evidence,
                  "retained_root": os.environ["BKMR_NATIVE_QUALIFICATION_ROOT"]})


def finish():
    evidence, fixture, workspace = owner(allow_partial=True)
    # Existing Actions processes this output even when finish later fails. Only
    # the presently requalified actual owner can make partial upload eligible.
    output_write({"retained_path": evidence,
                  "retained_root": os.environ["BKMR_NATIVE_QUALIFICATION_ROOT"]})
    errors = []
    preparation = os.environ.get("BKMR_NATIVE_PREPARATION_OUTCOME")
    if preparation != "success":
        errors.append("native preparation did not complete: " + str(preparation))
    retained = []
    results = []
    for case in CASES:
        path = evidence / ("case-" + case["id"] + ".result.json")
        try:
            result = json.loads(text(path)) if path.exists() else {**case, "accepted": False, "state": "incomplete/unexecuted"}
        except (OSError, ValueError) as error:
            errors.append(str(error))
            result = {**case, "accepted": False, "state": "actual result artifact unavailable"}
        results.append(result)
    for phase in ["candidate", "old"]:
        path = evidence / (phase + "-executables.json")
        if path.exists():
            try:
                artifacts = json.loads(text(path))["artifacts"]
            except (OSError, ValueError, KeyError) as error:
                errors.append(str(error))
                artifacts = {}
            for role, item in artifacts.items():
                try:
                    current = digest(item["path"])
                    if current["sha256"] != item["sha256"]:
                        raise ValueError("native executable changed after compiler observation: " + role)
                    retained.append(copy_artifact(item["path"], evidence / "executables" / role))
                except (OSError, ValueError) as error:
                    errors.append(str(error))
        else:
            errors.append(phase + " executable prerequisite missing; no binary qualification")
    # Only this fixture's declared material is retained, never private user data.
    material = []
    total = 0
    try:
        pending = [fixture] if fixture is not None else []
        visited = 0
        while pending:
            directory = pending.pop()
            with os.scandir(directory) as entries:
                for entry in entries:
                    visited += 1
                    if visited > 20000:
                        raise ValueError("owned material traversal capacity exceeded; no clipping/pass")
                    path = Path(entry.path)
                    relative = path.relative_to(fixture)
                    if relative.parts[0] in ["old-source", "old-target"]:
                        continue
                    if entry.is_dir(follow_symlinks=False):
                        if len(pending) >= 1024:
                            raise ValueError("owned material frontier capacity exceeded")
                        pending.append(path)
                        continue
                    if len(material) >= 8192:
                        raise ValueError("owned material file capacity exceeded")
                    if entry.is_symlink():
                        target = path.resolve(strict=True)
                        target.relative_to(fixture)
                        item = digest(target)
                        material.append({"path": str(relative), "symlink": os.readlink(path), "target": item})
                    else:
                        item = digest(path)
                        material.append({"path": str(relative), "observation": item})
                    total += item["bytes"]
                    if total > MATERIAL_LIMIT:
                        raise ValueError("owned material retention capacity exceeded; no clipping/pass")
        dump(evidence / "material-inventory.json", material)
        with tarfile.open(evidence / "native-material.tar", "w") as archive:
            for item in material:
                archive.add(fixture / item["path"], arcname=item["path"], recursive=False)
        retained.append(digest(evidence / "native-material.tar", MATERIAL_LIMIT + PARSE_LIMIT))
    except (OSError, ValueError) as error:
        errors.append(str(error))
    try:
        dump(evidence / "candidate-source-after.json", source_check(workspace))
        old = fixture / "old-source/bkmr-7.6.7/bkmr/Cargo.lock" if fixture is not None else None
        if old is not None and old.exists() and digest(old)["sha256"] != OLD_LOCK_SHA:
            raise ValueError("old unmodified lock changed during actual build")
    except (OSError, ValueError) as error:
        errors.append(str(error))
    leftovers = []
    try:
        leftovers = sorted(path.name for path in fixture.iterdir()
                           if path.name.startswith(("bkmr-literal-fts-", "bkmr-native-wal-", "bkmr-native-readonly-", "bkmr-backup-observation-"))) if fixture is not None else []
    except OSError as error:
        errors.append(str(error))
    if leftovers:
        errors.append("native case fixture cleanup incomplete: " + str(leftovers))
    prerequisites = {}
    for name in ["old-version", "candidate-version", "warm-create", "warm-add", "warm-info"]:
        path = evidence / (name + ".status")
        try:
            prerequisites[name] = int(text(path).strip()) if path.exists() else None
        except (OSError, ValueError) as error:
            errors.append(str(error))
            prerequisites[name] = None
    versions_ok = False
    path = evidence / "old-version.stdout"
    if path.exists():
        versions_ok = text(path).strip() == "bkmr 7.6.7"
    path = evidence / "candidate-version.stdout"
    current_version_ok = path.exists() and text(path).strip() == "bkmr 7.6.11"
    complete = (all(item["accepted"] for item in results) and not errors
                and (evidence / "compiled-rosters.json").exists()
                and all(value == 0 for value in prerequisites.values()) and versions_ok and current_version_ok)
    qualification = {"utc": now(), "accepted_selected_scope": complete,
         "selected_distinct_definitions": 24, "actual_accepted": sum(bool(r["accepted"]) for r in results),
         "cases": results, "prerequisites": prerequisites, "old_exact_version": versions_ok,
         "current_exact_version": current_version_ok, "preparation_outcome": preparation,
         "retained": retained, "errors": errors, "leftover_case_children": leftovers,
         "make_test": "unchanged ordinary step; actual platform step outcome/raw job log required separately",
         "limit": "not whole CI/provider/install/model acceptance; runtime SQLite version not inferred from helper Python"}
    # Outer status/EOF does not establish retirement of inner native children.
    # Keep even an accepted fixture for the existing finite hosted lifecycle;
    # no local recursive delete is warranted by this audit's observations.
    try:
        owner(allow_partial=True)
    except (OSError, ValueError, KeyError) as error:
        # Do not write through an affiliation that just failed. Invalidate the
        # upload locator; raw hosted failure remains when custody is unavailable.
        try:
            output_write({"retained_path": "", "retained_root": ""})
        except (OSError, ValueError, KeyError) as output_error:
            print("native retention output revocation failed: " + str(output_error), file=sys.stderr)
        print("native cleanup disposition unavailable; fixture not deleted: " + str(error),
              file=sys.stderr)
        raise
    try:
        dump(evidence / "cleanup.json", {"utc": now(), "removed_fixture": False,
             "retained_fixture": str(fixture) if fixture is not None else None,
             "current_affiliation_checked": True,
             "inner_process_retirement_witness": "unavailable; not inferred from outer status",
             "disposition": "preserved for existing hosted lifecycle retirement",
             "selected_scope_complete": complete,
             "limit": "host retirement is not observed here; no concurrent external mutation guarantee"})
    except OSError as error:
        errors.append(str(error))
        complete = False
        print("native cleanup metadata retention failed; fixture not deleted: " + str(error),
              file=sys.stderr)
    qualification["accepted_selected_scope"] = complete
    qualification["errors"] = errors
    dump(evidence / "qualification.json", qualification)
    if not complete:
        raise ValueError("native selected qualification incomplete or failed; actual partial artifacts retained")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "old-source", "compiler", "rosters", "case-start", "case-result", "retention-start", "finish"])
    parser.add_argument("argument", nargs="?")
    arguments = parser.parse_args()
    if arguments.command == "prepare":
        prepare()
    elif arguments.command == "old-source":
        old_source()
    elif arguments.command == "compiler":
        if arguments.argument not in ["candidate", "old"]:
            raise ValueError("explicit native compiler phase required")
        compiler(arguments.argument)
    elif arguments.command == "rosters":
        rosters()
    elif arguments.command == "case-start":
        case_start(arguments.argument)
    elif arguments.command == "case-result":
        case_result(arguments.argument)
    elif arguments.command == "retention-start":
        retention_start()
    else:
        finish()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, StopIteration, tarfile.TarError) as error:
        # The data audit fails honestly; no native exit/result is fabricated.
        try:
            evidence, _, _ = owner(allow_partial=True)
        except (OSError, ValueError, KeyError) as retention_error:
            if len(sys.argv) > 1 and sys.argv[1] in ["retention-start", "finish"]:
                try:
                    output_write({"retained_path": "", "retained_root": ""})
                except (OSError, ValueError, KeyError) as output_error:
                    print("native retention output revocation failed: " + str(output_error), file=sys.stderr)
            print("native audit error retention unavailable: " + str(retention_error), file=sys.stderr)
        else:
            try:
                with (evidence / "audit-errors.jsonl").open("a") as output:
                    output.write(json.dumps({"utc": now(), "command": sys.argv[1:],
                                             "actual_type": type(error).__name__, "message": str(error)}) + "\n")
            except OSError as retention_error:
                # Known custody still admits available raw artifacts for upload;
                # failure to append diagnostics does not erase that observation.
                print("native audit error append failed: " + str(retention_error), file=sys.stderr)
        print("native qualification artifact audit failed: " + str(error), file=sys.stderr)
        sys.exit(1)
