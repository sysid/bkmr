#!/usr/bin/env python
# ABOUTME: Claude Code PostToolUse hook that logs bkmr agent memory operations
# ABOUTME: Produces per-session structured JSONL and human-readable text logs
"""
Observability hook for bkmr agent memory operations.

Logs every bkmr command that an AI agent executes via Claude Code's Bash tool.
Non-bkmr commands are silently ignored (negligible overhead).

HOOK PAYLOAD (stdin JSON):
  {
    "session_id": "abc123",
    "cwd": "/Users/you/project",
    "tool_name": "Bash",
    "tool_input": { "command": "bkmr hsearch \"auth\" -t _mem_ --json --np" },
    "tool_result": "[ ... json output ... ]"
  }

LOG FILES (per session):
  ~/.claude/debug/bkmr-debug.<session_id>.log     Human-readable
  ~/.claude/debug/bkmr-debug.<session_id>.jsonl    Machine-readable JSONL

SETUP (global — works across all projects):
  1. chmod +x ~/dev/s/public/bkmr/skill/hooks/log-bkmr.py
  2. Add PostToolUse hook to ~/.claude/settings.json:
       {
         "hooks": {
           "PostToolUse": [
             {
               "matcher": "Bash",
               "hooks": [
                 {
                   "type": "command",
                   "command": "~/dev/s/public/bkmr/skill/hooks/log-bkmr.py"
                 }
               ]
             }
           ]
         }
       }
  3. Restart Claude Code session.

LIVE MONITORING:
  tail -f ~/.claude/debug/bkmr-debug.*.log
  tail -f ~/.claude/debug/bkmr-debug.*.log | grep WRITE

EXIT CODES:
  Always exits 0. Purely observational — never blocks agent behavior.
"""
import json
import logging
import sys
from pathlib import Path

DEBUG_DIR = Path.home() / ".claude" / "debug"


def setup_logging(session_id: str) -> logging.Logger:
    """Configure file-based logging for this session."""
    DEBUG_DIR.mkdir(parents=True, exist_ok=True)
    log_file = DEBUG_DIR / f"bkmr-debug.{session_id}.log"

    logger = logging.getLogger("log_bkmr")
    logger.setLevel(logging.DEBUG)

    # Avoid duplicate handlers on repeated calls
    if not logger.handlers:
        handler = logging.FileHandler(log_file)
        handler.setFormatter(
            logging.Formatter("%(asctime)s %(levelname)s %(message)s")
        )
        logger.addHandler(handler)

    return logger


def classify_operation(command: str) -> str:
    """Classify a bkmr command into a human-readable operation type."""
    if "hsearch" in command or "sem-search" in command:
        return "READ"
    if "search" in command:
        return "READ"
    if " add " in command:
        return "WRITE"
    if " update " in command:
        return "UPDATE"
    if " edit " in command:
        return "EDIT"
    if " delete " in command:
        return "DELETE"
    if " show " in command:
        return "SHOW"
    return "OTHER"


def main() -> None:
    try:
        payload = json.loads(sys.stdin.read())
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    command = payload.get("tool_input", {}).get("command", "")

    # Only log bkmr commands
    if not command.startswith("bkmr "):
        sys.exit(0)

    session_id = payload.get("session_id", "unknown")
    cwd = payload.get("cwd", "unknown")
    op = classify_operation(command)

    logger = setup_logging(session_id)
    logger.info("[%6s] %s (cwd=%s)", op, command, cwd)

    # Machine-readable JSONL (per session)
    jsonl_file = DEBUG_DIR / f"bkmr-debug.{session_id}.jsonl"
    entry = {
        "session_id": session_id,
        "op": op,
        "command": command,
        "cwd": cwd,
    }
    with jsonl_file.open("a") as f:
        f.write(json.dumps(entry) + "\n")


if __name__ == "__main__":
    main()
