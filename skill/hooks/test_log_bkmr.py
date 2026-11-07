"""Tests for the bkmr logging hook. Run: uv run --with pytest pytest skill/hooks/test_log_bkmr.py"""
import importlib.util
from pathlib import Path

import pytest

spec = importlib.util.spec_from_file_location("log_bkmr", Path(__file__).with_name("log-bkmr.py"))
log_bkmr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(log_bkmr)


@pytest.mark.parametrize(
    "command, operation",
    [
        ("bkmr hsearch 'x' -t _mem_ --json --np", "READ"),
        ("bkmr add 'x' 'gotcha' -t mem --no-web", "WRITE"),
        # Agents chain commands and set the database inline; these were not logged at all.
        ("cd /tmp/project && bkmr add 'x' 'gotcha' -t mem --no-web", "WRITE"),
        ("BKMR_DB_URL=/tmp/x.db bkmr hsearch 'x' -t _mem_", "READ"),
        ("/Users/me/bin/bkmr update --url 'x' 42", "UPDATE"),
        ("bkmr update 42 -d 'why' && printf 'y\\n' | bkmr delete 42", "UPDATE"),
        # The operation is the subcommand, not a word that happens to be in the content.
        ("bkmr add 'fix the search box' 'gotcha' -t mem", "WRITE"),
        ("bkmr show 42 --json", "SHOW"),
        ("bkmr delete 42", "DELETE"),
        ("printf 'y\\n' | bkmr delete 42", "DELETE"),
        # Unlike a plain delete of a memory, this cannot be undone
        ("bkmr delete --force 42", "PURGE"),
        ("bkmr delete 42 -f", "PURGE"),
        ("bkmr restore 7", "WRITE"),
        ("bkmr history 42 --json", "READ"),
        ("bkmr history", "READ"),
        ("bkmr verify 42 43", "VERIFY"),
        ("bkmr --version", "OTHER"),
    ],
)
def test_given_bkmr_command_when_classified_then_operation_is_its_subcommand(command, operation):
    assert log_bkmr.classify_operation(command) == operation


@pytest.mark.parametrize(
    "command",
    ["git status", "cd bkmr && ls", "ls ~/dev/bkmr/src", "cat skill/bkmr-memory/SKILL.md"],
)
def test_given_no_bkmr_invocation_when_checked_then_it_is_not_logged(command):
    assert not log_bkmr.invokes_bkmr(command)


@pytest.mark.parametrize(
    "command",
    ["bkmr --version", "cd x && bkmr add 'y'", "BKMR_DB_URL=/x bkmr search z", "/bin/bkmr show 1"],
)
def test_given_bkmr_invocation_when_checked_then_it_is_logged(command):
    assert log_bkmr.invokes_bkmr(command)
