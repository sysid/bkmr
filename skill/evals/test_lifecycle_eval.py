"""Tests for the lifecycle eval's command line. Run: make -C skill test"""

import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "lifecycle_eval", Path(__file__).with_name("lifecycle_eval.py")
)
lifecycle_eval = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lifecycle_eval)


def test_prompt_prints_the_users_request_and_the_store_path(tmp_path, capsys):
    lifecycle_eval.main(["prompt", str(tmp_path / "r1"), "stale-deploy-flag"])

    out = capsys.readouterr().out
    assert "deploy ledgerd to eu-west-1" in out
    assert str(tmp_path / "r1" / "mem.db") in out


def test_verbose_logs_debug_to_stderr_and_keeps_the_prompt_clean(tmp_path, capsys):
    # The prompt goes to an agent verbatim, so debug lines must never mix into stdout.
    lifecycle_eval.main(["-v", "prompt", str(tmp_path / "r1"), "stale-deploy-flag"])

    captured = capsys.readouterr()
    assert "DEBUG" in captured.err
    assert "DEBUG" not in captured.out
