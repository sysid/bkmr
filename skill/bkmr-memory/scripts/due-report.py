#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""List agent memories that are due for review. Report only: it never changes the store.

  due-report.py [--db FILE] [--json]      Ask bkmr for all memories and list the due ones.
  due-report.py --bkmr ~/bin/bkmr ...     Pick the binary: in the bkmr repo, PATH may find an
                                          older debug build without verified_at.
  due-report.py --input FILE [--json]     Read a saved `bkmr search -t _mem_ --json` instead.

A memory is due when its age reaches its review window. Age counts from `verified_at`, the last
`bkmr verify`, or from `created_at` if it was never stamped or changed since. Due means "audit",
never "delete": the audit decides what to do (SKILL.md, "Keeping Memory Current").

Windows by class: fact 90 days, procedure, gotcha and episode 180, preference 365. A memory
pinned to a tool version or an agent harness uses at most 90 days, whatever its class, because
those go stale when the tool changes, not with age.
"""

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone

WINDOW_DAYS = {"fact": 90, "procedure": 180, "gotcha": 180, "episode": 180, "preference": 365}
UNCLASSIFIED_WINDOW_DAYS = 90
PINNED_WINDOW_DAYS = 90

# Tags that tie a memory to an agent harness, which changes with every harness release.
HARNESS_TAGS = {"sandbox", "seatbelt", "claude-code", "copilot-cli", "codex", "mcp", "harness"}

# ponytail: version detection is a regex. It catches three-part versions, v-prefixed versions and
# release candidates, and misses two-part ones like "fastembed 5" or "Python 3.11", which look
# like any other number. Upgrade path: a per-memory pin field if misses show up in audits.
VERSION = re.compile(r"\bv\d+(?:\.\d+)*\b|\b\d+\.\d+\.\d+\b|-rc\.\d+")

# Evidence an agent can recheck: a file path with an extension, a commit hash, or a URL.
FILE_PATH = re.compile(r"\b[\w.-]+/[\w./-]*\w\.[A-Za-z]\w*\b")
COMMIT = re.compile(r"\b(?=[0-9a-f]*\d)(?=[0-9a-f]*[a-f])[0-9a-f]{7,40}\b")
URL = re.compile(r"https?://\S+")


def parse_time(value):
    parsed = datetime.fromisoformat(value)
    return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)


def review(memory, now):
    """Return the review entry for one memory, whether due or not."""
    classes = sorted(t for t in memory["tags"] if t in WINDOW_DAYS)
    flags = []
    if not classes:
        window = UNCLASSIFIED_WINDOW_DAYS
        flags.append("no class")
    else:
        window = min(WINDOW_DAYS[c] for c in classes)
        if len(classes) > 1:
            flags.append("classes: " + ",".join(classes))

    pins = []
    if VERSION.search(memory["url"]) or VERSION.search(memory["title"]):
        pins.append("version")
    pins += sorted(t for t in memory["tags"] if t in HARNESS_TAGS)
    if pins:
        window = min(window, PINNED_WINDOW_DAYS)
        flags.append("pinned: " + ",".join(pins))

    if memory["verified_at"]:
        anchor, anchor_time = "verified", parse_time(memory["verified_at"])
    else:
        anchor, anchor_time = "created", parse_time(memory["created_at"])
    age = (now - anchor_time).days

    text = f"{memory['url']}\n{memory['title']}\n{memory.get('description') or ''}"
    checkable = any(p.search(text) for p in (FILE_PATH, COMMIT, URL))

    return {
        "id": memory["id"],
        "title": memory["title"],
        "classes": classes,
        "anchor": anchor,
        "anchor_date": anchor_time.date().isoformat(),
        "age_days": age,
        "window_days": window,
        "overdue_days": age - window,
        "checkable": checkable,
        "flags": flags,
    }


def due_memories(memories, now):
    """Due memories, most overdue first."""
    entries = [review(m, now) for m in memories]
    due = [e for e in entries if e["overdue_days"] >= 0]
    return sorted(due, key=lambda e: (-e["overdue_days"], e["id"]))


def load_from_bkmr(bkmr, db):
    command = [bkmr, "search", "-t", "_mem_", "--json", "--np"]
    if db:
        command += ["--db", db]
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    return json.loads(result.stdout)


def print_table(due, total):
    print(f"{len(due)} of {total} memories due")
    if not due:
        return
    print(f"{'id':>6} {'over':>5} {'win':>4} {'from':<8} {'anchor':<10} chk  title / flags")
    for e in due:
        check = "yes" if e["checkable"] else "-"
        print(
            f"{e['id']:>6} {e['overdue_days']:>5} {e['window_days']:>4} {e['anchor']:<8} "
            f"{e['anchor_date']:<10} {check:<4} {e['title']}"
        )
        if e["flags"]:
            print(f"{'':>42}{'; '.join(e['flags'])}")


def main(argv=None):
    parser = argparse.ArgumentParser(description="List agent memories due for review.")
    parser.add_argument("--bkmr", default="bkmr", help="bkmr binary (default: bkmr on PATH)")
    parser.add_argument("--db", help="bkmr database (default: BKMR_DB_URL or config.toml)")
    parser.add_argument(
        "--input", help="read a saved `bkmr search -t _mem_ --json` instead of calling bkmr"
    )
    parser.add_argument("--json", action="store_true", help="print JSON for the audit")
    parser.add_argument("--now", help="reference date, YYYY-MM-DD (default: today)")
    args = parser.parse_args(argv)

    if args.input:
        with open(args.input) as f:
            memories = json.load(f)
    else:
        memories = load_from_bkmr(args.bkmr, args.db)

    if memories and "verified_at" not in memories[0]:
        print(
            "error: memories have no verified_at; bkmr 8.2.0 or later is required", file=sys.stderr
        )
        return 1

    now = parse_time(args.now) if args.now else datetime.now(timezone.utc)
    due = due_memories(memories, now)
    if args.json:
        print(json.dumps(due, indent=2))
    else:
        print_table(due, len(memories))
    return 0


if __name__ == "__main__":
    sys.exit(main())
