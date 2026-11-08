"""Tests for the due report. Run: uv run --with pytest pytest skill/bkmr-memory/scripts/test_due_report.py"""

import importlib.util
import json
from datetime import datetime, timezone
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "due_report", Path(__file__).with_name("due-report.py")
)
due_report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(due_report)

NOW = datetime(2026, 11, 8, tzinfo=timezone.utc)


def memory(
    id=1,
    tags=("_mem_", "gotcha"),
    created_at="2026-01-01T00:00:00+00:00",
    verified_at=None,
    url="Plain claim without any version.",
    title="A title",
    description="",
):
    """A memory as `bkmr search --json` returns it, with only the fields the report reads."""
    return {
        "id": id,
        "url": url,
        "title": title,
        "description": description,
        "tags": list(tags),
        "created_at": created_at,
        "verified_at": verified_at,
    }


def ids(report):
    return [entry["id"] for entry in report]


# --- review windows by class ---------------------------------------------------------------------


def test_a_gotcha_younger_than_180_days_is_not_due():
    young = memory(tags=["_mem_", "gotcha"], created_at="2026-06-01T00:00:00+00:00")  # 160 days

    assert due_report.due_memories([young], NOW) == []


def test_a_gotcha_older_than_180_days_is_due():
    old = memory(tags=["_mem_", "gotcha"], created_at="2026-04-01T00:00:00+00:00")  # 221 days

    [entry] = due_report.due_memories([old], NOW)

    assert entry["window_days"] == 180
    assert entry["age_days"] == 221
    assert entry["overdue_days"] == 41


def test_a_fact_is_due_after_90_days_and_a_preference_only_after_365():
    fact = memory(id=1, tags=["_mem_", "fact"], created_at="2026-07-01T00:00:00+00:00")  # 130 days
    preference = memory(
        id=2, tags=["_mem_", "preference"], created_at="2026-01-01T00:00:00+00:00"
    )  # 311

    assert ids(due_report.due_memories([fact, preference], NOW)) == [1]


def test_procedure_and_episode_use_180_days():
    procedure = memory(
        id=1, tags=["_mem_", "procedure"], created_at="2026-05-01T00:00:00+00:00"
    )  # 191
    episode = memory(id=2, tags=["_mem_", "episode"], created_at="2026-06-01T00:00:00+00:00")  # 160

    assert ids(due_report.due_memories([procedure, episode], NOW)) == [1]


def test_a_memory_with_two_classes_gets_the_shorter_window():
    both = memory(
        tags=["_mem_", "gotcha", "fact"], created_at="2026-07-01T00:00:00+00:00"
    )  # 130 days

    [entry] = due_report.due_memories([both], NOW)

    assert entry["window_days"] == 90
    assert "classes: fact,gotcha" in entry["flags"]


def test_a_memory_without_a_class_gets_90_days_and_is_flagged():
    unclassified = memory(tags=["_mem_", "kafka"], created_at="2026-07-01T00:00:00+00:00")

    [entry] = due_report.due_memories([unclassified], NOW)

    assert entry["window_days"] == 90
    assert "no class" in entry["flags"]


# --- anchor: verified_at, else created_at ------------------------------------------------------


def test_a_recent_stamp_keeps_an_old_memory_from_being_due():
    stamped = memory(
        created_at="2026-01-01T00:00:00+00:00", verified_at="2026-10-01T00:00:00+00:00"
    )

    assert due_report.due_memories([stamped], NOW) == []


def test_the_report_says_which_date_it_counted_from():
    stamped = memory(
        created_at="2025-01-01T00:00:00+00:00", verified_at="2026-03-01T00:00:00+00:00"
    )
    unstamped = memory(id=2, created_at="2026-03-01T00:00:00+00:00")

    by_id = {e["id"]: e for e in due_report.due_memories([stamped, unstamped], NOW)}

    assert by_id[1]["anchor"] == "verified"
    assert by_id[2]["anchor"] == "created"


# --- pinned to a version or harness: at most 90 days -------------------------------------------


def test_a_gotcha_pinning_a_version_uses_90_days():
    pinned = memory(
        url="ort-sys v2.0.0-rc.11 needs onnxruntime 1.24.x", created_at="2026-07-01T00:00:00+00:00"
    )

    [entry] = due_report.due_memories([pinned], NOW)

    assert entry["window_days"] == 90
    assert "pinned: version" in entry["flags"]


def test_a_version_in_the_title_also_pins():
    pinned = memory(
        title="skim 4.4.0 preview_fn does not repaint", created_at="2026-07-01T00:00:00+00:00"
    )

    assert ids(due_report.due_memories([pinned], NOW)) == [1]


def test_a_gotcha_tagged_with_a_harness_uses_90_days():
    harness = memory(tags=["_mem_", "gotcha", "sandbox"], created_at="2026-07-01T00:00:00+00:00")

    [entry] = due_report.due_memories([harness], NOW)

    assert entry["window_days"] == 90
    assert "pinned: sandbox" in entry["flags"]


def test_numbers_that_are_not_versions_do_not_pin():
    # Cosine 0.95, a port, a date and "fastembed 5": none is a three-part or v-prefixed version.
    plain = memory(
        url="Similarity 0.95 on port 5433 since 2026-04-01 with fastembed 5.",
        created_at="2026-07-01T00:00:00+00:00",
    )

    assert due_report.due_memories([plain], NOW) == []


def test_pinning_never_lengthens_a_shorter_window():
    pinned_fact = memory(tags=["_mem_", "fact", "sandbox"], created_at="2026-07-01T00:00:00+00:00")

    [entry] = due_report.due_memories([pinned_fact], NOW)

    assert entry["window_days"] == 90


# --- checkable evidence ------------------------------------------------------------------------


def test_a_memory_citing_a_repo_path_is_checkable():
    cited = memory(url="The retry logic in `src/http/client.rs:45` uses backoff.")

    [entry] = due_report.due_memories([cited], NOW)

    assert entry["checkable"] is True


def test_provenance_in_the_description_makes_it_checkable():
    cited = memory(description="Corrected: config/db.yml sets 5433, checked 2026-11-07")

    [entry] = due_report.due_memories([cited], NOW)

    assert entry["checkable"] is True


def test_a_commit_hash_makes_it_checkable():
    cited = memory(url="hsearch quoting changed in d749db7.")

    [entry] = due_report.due_memories([cited], NOW)

    assert entry["checkable"] is True


def test_a_memory_about_live_infrastructure_with_no_citation_is_not_checkable():
    uncited = memory(url="The prod Kafka cluster rebalances slowly after a broker restart.")

    [entry] = due_report.due_memories([uncited], NOW)

    assert entry["checkable"] is False


# --- ordering and output ------------------------------------------------------------------------


def test_the_most_overdue_memory_comes_first():
    a_bit = memory(id=1, created_at="2026-04-20T00:00:00+00:00")
    a_lot = memory(id=2, created_at="2026-01-01T00:00:00+00:00")

    assert ids(due_report.due_memories([a_bit, a_lot], NOW)) == [2, 1]


def test_json_output_lists_due_memories_from_a_saved_search(tmp_path, capsys):
    saved = tmp_path / "memories.json"
    saved.write_text(
        json.dumps(
            [
                memory(id=7, created_at="2026-01-01T00:00:00+00:00"),
                memory(id=8, created_at="2026-11-01T00:00:00+00:00"),
            ]
        )
    )

    exit_code = due_report.main(["--input", str(saved), "--json", "--now", "2026-11-08"])

    assert exit_code == 0
    assert ids(json.loads(capsys.readouterr().out)) == [7]


def test_a_chosen_bkmr_binary_is_asked_for_all_memories(tmp_path, capsys):
    # Inside the bkmr repo, PATH can resolve `bkmr` to an older debug build, so the binary is selectable.
    fake_bkmr = tmp_path / "bkmr"
    fake_bkmr.write_text(
        f'#!/bin/sh\necho "$@" > {tmp_path / "args"}\ncat {tmp_path / "memories.json"}\n'
    )
    fake_bkmr.chmod(0o755)
    (tmp_path / "memories.json").write_text(
        json.dumps([memory(id=7, created_at="2026-01-01T00:00:00+00:00")])
    )

    due_report.main(["--bkmr", str(fake_bkmr), "--db", "/x/bm.db", "--json", "--now", "2026-11-08"])

    assert ids(json.loads(capsys.readouterr().out)) == [7]
    assert (tmp_path / "args").read_text().split() == [
        "search",
        "-t",
        "_mem_",
        "--json",
        "--np",
        "--db",
        "/x/bm.db",
    ]


def test_an_old_bkmr_without_verified_at_is_refused(tmp_path, capsys):
    saved = tmp_path / "memories.json"
    old_shape = memory()
    del old_shape["verified_at"]
    saved.write_text(json.dumps([old_shape]))

    exit_code = due_report.main(["--input", str(saved)])

    assert exit_code == 1
    assert "8.2.0" in capsys.readouterr().err


def test_the_table_shows_a_summary_line(tmp_path, capsys):
    saved = tmp_path / "memories.json"
    saved.write_text(
        json.dumps(
            [
                memory(id=7, created_at="2026-01-01T00:00:00+00:00"),
                memory(id=8, created_at="2026-11-01T00:00:00+00:00"),
            ]
        )
    )

    due_report.main(["--input", str(saved), "--now", "2026-11-08"])

    out = capsys.readouterr().out
    assert "1 of 2 memories due" in out
    assert "7" in out


# --- debug logging -----------------------------------------------------------------------------------


def test_verbose_logs_why_each_memory_is_due_to_stderr(tmp_path, capsys):
    saved = tmp_path / "memories.json"
    saved.write_text(json.dumps([memory(id=7, created_at="2026-01-01T00:00:00+00:00")]))

    due_report.main(["--input", str(saved), "--json", "--now", "2026-11-08", "-v"])

    captured = capsys.readouterr()
    assert "DEBUG" in captured.err
    assert "#7" in captured.err and "window 180" in captured.err
    # stdout stays machine-readable for the audit
    assert ids(json.loads(captured.out)) == [7]


def test_without_verbose_there_is_no_debug_output(tmp_path, capsys):
    saved = tmp_path / "memories.json"
    saved.write_text(json.dumps([memory(id=7, created_at="2026-01-01T00:00:00+00:00")]))

    due_report.main(["--input", str(saved), "--json", "--now", "2026-11-08"])

    assert "DEBUG" not in capsys.readouterr().err
