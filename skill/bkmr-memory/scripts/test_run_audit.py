"""Tests for the audit runner. Run: uv run --with pytest pytest skill/bkmr-memory/scripts/test_run_audit.py"""

import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("run_audit", Path(__file__).with_name("run-audit.py"))
run_audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run_audit)


def due_entry(id, title="A title", overdue=10):
    return {
        "id": id,
        "title": title,
        "classes": ["gotcha"],
        "anchor": "created",
        "anchor_date": "2026-04-01",
        "age_days": 190,
        "window_days": 180,
        "overdue_days": overdue,
        "checkable": False,
        "flags": [],
    }


# --- stop threshold: more than 5 changes or 10% of those reviewed ------------------------------


def test_the_change_limit_is_ten_percent_of_reviewed_memories():
    assert run_audit.change_limit(20) == 2
    assert run_audit.change_limit(40) == 4


def test_the_change_limit_never_exceeds_five():
    assert run_audit.change_limit(200) == 5


def test_a_small_audit_may_still_change_one_memory():
    assert run_audit.change_limit(3) == 1


# --- what the agent may do -----------------------------------------------------------------------


def test_the_agent_is_denied_everything_not_explicitly_allowed():
    command = run_audit.claude_command("prompt", model="m", budget_usd=5)

    assert command[command.index("--permission-mode") + 1] == "dontAsk"
    assert "--strict-mcp-config" in command


def test_the_agent_may_read_and_use_the_memory_commands_but_not_open_or_edit():
    command = run_audit.claude_command("prompt", model="m", budget_usd=5)
    allowed = command[command.index("--allowedTools") + 1].split(",")

    assert "Read" in allowed
    assert "Bash(bkmr update:*)" in allowed
    assert "Bash(bkmr verify:*)" in allowed
    assert "Bash(bkmr delete --yes:*)" in allowed
    # `bkmr open` runs shell bookmarks; Write and Edit would let it change files, not memories.
    assert not any(tool.startswith("Bash(bkmr open") for tool in allowed)
    assert "Write" not in allowed and "Edit" not in allowed
    assert not any(tool == "Bash" or tool.startswith("Bash(git push") for tool in allowed)


def test_user_and_project_settings_are_not_loaded():
    # User settings may allow plain `Bash`; loaded, they would widen the allowed tools to anything.
    command = run_audit.claude_command("prompt", model="m", budget_usd=5)

    assert command[command.index("--setting-sources") + 1] == "local"


def test_the_budget_caps_the_run():
    command = run_audit.claude_command("prompt", model="m", budget_usd=3)

    assert command[command.index("--max-budget-usd") + 1] == "3"


# --- prompt ----------------------------------------------------------------------------------------


def test_the_prompt_names_each_due_memory_and_the_change_limit():
    prompt = run_audit.audit_prompt(
        [due_entry(3505), due_entry(3516)], limit=1, skill_path="/s/SKILL.md"
    )

    assert '"id": 3505' in prompt and '"id": 3516' in prompt
    assert "more than 1" in prompt
    assert "/s/SKILL.md" in prompt


# --- digest built from the store, not from what the agent says ------------------------------------


def test_the_digest_counts_changes_and_deletes_from_history_and_lists_restore_commands():
    history = [
        {"history_id": 11, "bookmark_id": 3505, "change": "update", "title": "ort-sys pin"},
        {"history_id": 12, "bookmark_id": 3501, "change": "delete", "title": "junk"},
    ]

    summary = run_audit.summarize(history, stamped=[3504, 3513], limit=2)

    assert summary["changed"] == [3505]
    assert summary["deleted"] == [3501]
    assert summary["stamped"] == [3504, 3513]
    assert summary["exceeded"] is False
    assert "bkmr restore 11" in summary["restore"][0]


def test_several_saves_of_one_memory_count_as_one_change():
    # A correction followed by a reason-only update leaves two history rows for one memory.
    history = [
        {"history_id": 11, "bookmark_id": 3505, "change": "update", "title": "x"},
        {"history_id": 12, "bookmark_id": 3505, "change": "update", "title": "x"},
    ]

    assert run_audit.summarize(history, stamped=[], limit=1)["changed"] == [3505]


def test_a_reason_saved_before_a_delete_counts_only_as_the_delete():
    history = [
        {"history_id": 11, "bookmark_id": 3501, "change": "update", "title": "junk"},
        {"history_id": 12, "bookmark_id": 3501, "change": "delete", "title": "junk"},
    ]

    summary = run_audit.summarize(history, stamped=[], limit=1)

    assert summary["changed"] == []
    assert summary["deleted"] == [3501]


def test_more_changes_than_the_limit_are_flagged():
    history = [
        {"history_id": i, "bookmark_id": 100 + i, "change": "update", "title": "x"}
        for i in range(3)
    ]

    assert run_audit.summarize(history, stamped=[], limit=2)["exceeded"] is True


# --- end to end with stand-ins for bkmr and claude --------------------------------------------------


def write_script(path, body):
    path.write_text("#!/bin/sh\n" + body)
    path.chmod(0o755)


def test_a_run_writes_a_digest_with_the_agents_report_and_the_measured_changes(tmp_path):
    memories = [
        {
            "id": 7,
            "url": "claim",
            "title": "Old gotcha",
            "description": "",
            "tags": ["_mem_", "gotcha"],
            "created_at": "2026-01-01T00:00:00+00:00",
            "verified_at": None,
        }
    ]
    (tmp_path / "memories.json").write_text(json.dumps(memories))
    (tmp_path / "history-before.json").write_text(json.dumps([{"history_id": 4}]))
    (tmp_path / "history-after.json").write_text(
        json.dumps(
            [
                {"history_id": 5, "bookmark_id": 7, "change": "update", "title": "Old gotcha"},
                {"history_id": 4},
            ]
        )
    )
    # The fake bkmr answers `history -l 1` with the state before the run, and later calls with the state after.
    write_script(
        tmp_path / "bkmr",
        f"""
case "$1 $2 $3" in
  "history -l 1") cat {tmp_path}/history-before.json ;;
  history*) cat {tmp_path}/history-after.json ;;
  search*) cat {tmp_path}/memories.json ;;
esac
""",
    )
    write_script(
        tmp_path / "claude",
        'echo \'{"is_error": false, "result": "Corrected #7: evidence X.", "total_cost_usd": 0.4}\'\n',
    )

    exit_code = run_audit.main(
        [
            "--bkmr",
            str(tmp_path / "bkmr"),
            "--claude",
            str(tmp_path / "claude"),
            "--db",
            str(tmp_path / "bm.db"),
            "--out-dir",
            str(tmp_path / "out"),
            "--now",
            "2026-11-08",
        ]
    )

    digest = (tmp_path / "out" / "2026-11-08.md").read_text()
    assert exit_code == 0
    assert "Corrected #7: evidence X." in digest
    assert "Changed: 7" in digest
    assert "bkmr restore 5" in digest


def test_nothing_due_skips_the_agent(tmp_path):
    memories = [
        {
            "id": 7,
            "url": "claim",
            "title": "Young",
            "description": "",
            "tags": ["_mem_", "gotcha"],
            "created_at": "2026-11-01T00:00:00+00:00",
            "verified_at": None,
        }
    ]
    (tmp_path / "memories.json").write_text(json.dumps(memories))
    write_script(tmp_path / "bkmr", f"cat {tmp_path}/memories.json\n")
    write_script(tmp_path / "claude", f"touch {tmp_path}/claude-was-called\n")

    exit_code = run_audit.main(
        [
            "--bkmr",
            str(tmp_path / "bkmr"),
            "--claude",
            str(tmp_path / "claude"),
            "--db",
            str(tmp_path / "bm.db"),
            "--out-dir",
            str(tmp_path / "out"),
            "--now",
            "2026-11-08",
        ]
    )

    assert exit_code == 0
    assert not (tmp_path / "claude-was-called").exists()
    assert "Nothing due" in (tmp_path / "out" / "2026-11-08.md").read_text()


def test_an_agent_error_fails_the_run(tmp_path):
    memories = [
        {
            "id": 7,
            "url": "claim",
            "title": "Old",
            "description": "",
            "tags": ["_mem_", "gotcha"],
            "created_at": "2026-01-01T00:00:00+00:00",
            "verified_at": None,
        }
    ]
    (tmp_path / "memories.json").write_text(json.dumps(memories))
    write_script(
        tmp_path / "bkmr",
        f"""
case "$1" in
  history) echo '[]' ;;
  search) cat {tmp_path}/memories.json ;;
esac
""",
    )
    write_script(
        tmp_path / "claude", 'echo \'{"is_error": true, "result": "Credit balance is too low"}\'\n'
    )

    exit_code = run_audit.main(
        [
            "--bkmr",
            str(tmp_path / "bkmr"),
            "--claude",
            str(tmp_path / "claude"),
            "--db",
            str(tmp_path / "bm.db"),
            "--out-dir",
            str(tmp_path / "out"),
            "--now",
            "2026-11-08",
        ]
    )

    assert exit_code == 1
    assert "Credit balance is too low" in (tmp_path / "out" / "2026-11-08.md").read_text()
