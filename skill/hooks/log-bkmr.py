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
import re
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


# An invocation is the word bkmr at a command position or after a path, never inside a path such as
# "cd bkmr" or "dev/bkmr/src". Text that merely quotes "bkmr add" is also logged: this is a
# statistic, and missing real chained commands would cost more.
BKMR_INVOCATION = re.compile(r"(?:^|[\s;&|(/])bkmr\s+(--?\w[\w-]*|[a-z][a-z-]*)")

OPERATIONS = {
    "hsearch": "READ",
    "sem-search": "READ",
    "search": "READ",
    "add": "WRITE",
    "update": "UPDATE",
    "edit": "EDIT",
    "delete": "DELETE",
    "restore": "WRITE",
    "history": "READ",
    "show": "SHOW",
    "verify": "VERIFY",
}

FORCE_FLAG = re.compile(r"\s(--force|-f)(\s|$)")


def invokes_bkmr(command: str) -> bool:
    """True if the command runs bkmr, also when chained or with a path or environment prefix."""
    return BKMR_INVOCATION.search(command) is not None


def classify_operation(command: str) -> str:
    """Classify a command by the subcommand of its first bkmr invocation."""
    invocation = BKMR_INVOCATION.search(command)
    if not invocation:
        return "OTHER"
    operation = OPERATIONS.get(invocation.group(1), "OTHER")
    # Unlike a plain delete of a memory, this cannot be undone
    if operation == "DELETE" and FORCE_FLAG.search(command):
        return "PURGE"
    return operation


def main() -> None:
    try:
        payload = json.loads(sys.stdin.read())
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    command = payload.get("tool_input", {}).get("command", "")

    # Only log bkmr commands
    if not invokes_bkmr(command):
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
