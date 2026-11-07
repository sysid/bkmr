#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Fixture and deterministic grader for the memory lifecycle evals in evals.json.

  lifecycle_eval.py setup <dir>          Build <dir>/repo and <dir>/mem.db. Never touches a real DB.
  lifecycle_eval.py prompt <dir> <eval>  Print the prompt for an agent run, with the store path.
  lifecycle_eval.py grade <dir> <eval>   Grade the end state; prints JSON in the README's schema.

Each run needs its own <dir>. An agent gets only the printed prompt: the eval's name, this file and
the expected outcomes must stay out of its view, or the eval tests nothing.

The stale-consistent rate is the share of runs of the "stale" evals that failed `acted_on_truth`.
Set BKMR_BIN to choose the binary; the default is `bkmr` from PATH.
"""
import json
import os
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

BIN = os.environ.get("BKMR_BIN", "bkmr")

FILES = {
    "deploy.sh": """#!/usr/bin/env bash
# Deploys ledgerd. Authentication comes from the OIDC role since 2.0.
set -euo pipefail
region=""
while [ $# -gt 0 ]; do
  case "$1" in
    --region) region="$2"; shift 2 ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
done
[ -n "$region" ] || { echo "--region is required" >&2; exit 2; }
echo "deploying ledgerd to $region"
""",
    "Makefile": "test:\n\tpython3 -m unittest discover -s tests\n",
    "requirements.txt": "orjson==3.9.15\n",
    "src/ledgerd/__init__.py": "",
    "src/ledgerd/config.py": "DEFAULT_TIMEOUT = 30\nPORT = 8123\n",
    "src/ledgerd/logging_setup.py": 'LOG_PATH = "/var/log/ledgerd/app.log"\n',
    "src/ledgerd/amounts.py": 'def parse_amount(text):\n    return round(float(text.replace(",", "")), 2)\n',
    "src/ledgerd/migrate.py": """import hashlib
from pathlib import Path

SHIPPED = {"0001_init.sql": "%s"}


def check_shipped(directory):
    # A shipped migration must never change: production compares this checksum at startup.
    for name, digest in SHIPPED.items():
        actual = hashlib.sha256(Path(directory, name).read_bytes()).hexdigest()
        if actual != digest:
            raise RuntimeError(f"shipped migration {name} was edited")
""",
    "migrations/0001_init.sql": "CREATE TABLE ledger (id INTEGER PRIMARY KEY, amount NUMERIC NOT NULL);\n",
    "tests/__init__.py": "",
    # The suite really opens port 8123, so one clean run cannot disprove the flake memory.
    "tests/test_server.py": """import http.server
import sys
import unittest

sys.path.insert(0, "src")
from ledgerd import config


class ServerTest(unittest.TestCase):
    def test_binds_configured_port(self):
        server = http.server.HTTPServer(("127.0.0.1", config.PORT), http.server.BaseHTTPRequestHandler)
        self.addCleanup(server.server_close)
        self.assertEqual(server.server_address[1], 8123)
""",
    "tests/test_config.py": """import sys
import unittest

sys.path.insert(0, "src")
from ledgerd import config


class ConfigTest(unittest.TestCase):
    def test_port(self):
        self.assertEqual(config.PORT, 8123)
""",
}

# key: (classification tags, title, content)
MEMORIES = {
    "stale_flag": (
        "gotcha,deployment,project:ledgerd",
        "ledgerd deploy needs the legacy auth flag",
        "`deploy.sh` must be called with `--legacy-auth`, otherwise the deploy aborts with an auth\n"
        "error against the old token service.\n\n```bash\n./deploy.sh --legacy-auth --region <region>\n```",
    ),
    "stale_guard": (
        "gotcha,config,project:ledgerd",
        "Do not change DEFAULT_TIMEOUT in ledgerd",
        "Never change `DEFAULT_TIMEOUT` in `src/ledgerd/config.py`: `src/ledgerd/legacy_guard.py`\n"
        "asserts at startup that it equals 30, so any other value stops the service from booting.",
    ),
    "valid_migration": (
        "gotcha,database,project:ledgerd",
        "Shipped ledgerd migrations are immutable",
        "Never edit a migration that has shipped: `src/ledgerd/migrate.py` compares its checksum at\n"
        "startup and refuses to run. Add a new numbered migration instead.",
    ),
    "orjson_pin": (
        "fact,dependencies,project:ledgerd",
        "ledgerd pins orjson below 3.10",
        "ledgerd pins `orjson` to 3.9.15 because 3.10.0 changed how it serialises dates.",
    ),
    "log_path": (
        "fact,logging,project:ledgerd",
        "ledgerd log file location",
        "ledgerd writes its log to `/var/log/ledgerd/app.log`, set by `LOG_PATH` in\n"
        "`src/ledgerd/logging_setup.py`.",
    ),
    "flaky_tests": (
        "gotcha,testing,project:ledgerd",
        "ledgerd make test can fail on port 8123",
        "`make test` sometimes fails with `address already in use` on port 8123 when an earlier test\n"
        "server was not closed. It is intermittent: wait a minute and run it again.",
    ),
    "pytest_pref": (
        "preference,testing,python",
        "Python testing preference",
        "User prefers pytest over unittest for new Python tests.",
    ),
    "noise_vpn": (
        "fact,infrastructure,project:ledgerd",
        "ledgerd staging is only reachable over VPN",
        "The ledgerd staging environment is only reachable over the corporate VPN.",
    ),
}

PROMPTS = {
    "stale-deploy-flag": "Give me the exact command to deploy ledgerd to eu-west-1. Do not run it.",
    "stale-missing-guard": "Our requests time out too early. Change the default timeout in ledgerd to 60 seconds.",
    "valid-constraint-holds": "Add a `currency` column (TEXT, not null, default 'EUR') to the ledger table.",
    "supersede-on-write": "We upgraded ledgerd to orjson 3.10.6, which fixes the date serialisation problem, "
    "and requirements.txt will be updated separately. Remember this for future sessions.",
    "change-trigger": "Change the ledgerd log file to /var/log/ledgerd/service.log.",
    "not-reproduced-keeps-memory": "Run make test and tell me whether the suite is healthy. "
    "If it passes, do whatever is appropriate with what we know about this suite.",
    "preference-not-overruled": "Add a unit test for parse_amount in src/ledgerd/amounts.py. "
    "I notice the existing tests in this repo use unittest.",
}

# Evals whose acted_on_truth result counts towards the stale-consistent rate.
STALE_EVALS = {"stale-deploy-flag", "stale-missing-guard"}


def sh(*args, cwd=None, env=None):
    return subprocess.run(args, cwd=cwd, env=env, check=True, capture_output=True, text=True, stdin=subprocess.DEVNULL)


def setup(base: Path):
    repo = base / "repo"
    if repo.exists():
        sys.exit(f"{repo} exists: every run needs its own directory")
    import hashlib

    migration = FILES["migrations/0001_init.sql"].encode()
    for name, text in FILES.items():
        path = repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        if name == "src/ledgerd/migrate.py":
            text = text % hashlib.sha256(migration).hexdigest()
        path.write_text(text)
    (repo / "deploy.sh").chmod(0o755)
    identity = ["-c", "user.name=fixture", "-c", "user.email=fixture@example.com"]
    sh("git", "init", "-q", str(repo))
    sh("git", *identity, "add", "-A", cwd=repo)
    sh("git", *identity, "commit", "-q", "-m", "ledgerd 2.0: OIDC auth, drop the token service", cwd=repo)

    db = base / "mem.db"
    env = {**os.environ, "BKMR_DB_URL": str(db)}
    sh(BIN, "create-db", str(db), env=env)
    ids = {}
    for key, (tags, title, content) in MEMORIES.items():
        sh(BIN, "add", content, tags, "--title", title, "-t", "mem", "--no-web", env=env)
        ids[key] = sqlite3.connect(db).execute("SELECT max(id) FROM bookmarks").fetchone()[0]
    (base / "memories.json").write_text(json.dumps(ids))


def prompt(base: Path, name: str):
    return f"""You are working in the repository {base / 'repo'} (change directory there first).

Your persistent memory is a bkmr store. For every bkmr command use exactly this prefix, because
any other store is off limits for this task:

    BKMR_DB_URL={base / 'mem.db'} {BIN} ...

First read {base.parent / 'SKILL.md'} and follow it as your memory skill. Do not use the Skill tool
for this, and do not read anything outside the repository apart from that file.

Do not commit. When you are done, write your final reply to the user to {base / 'answer.md'} and
return the same text.

The user's request:

{PROMPTS[name]}"""


class State:
    def __init__(self, base: Path):
        self.repo = base / "repo"
        self.ids = json.loads((base / "memories.json").read_text())
        self.db = sqlite3.connect(f"file:{base / 'mem.db'}?mode=ro", uri=True)
        answer = base / "answer.md"
        self.answer = answer.read_text() if answer.exists() else ""

    def memory(self, key):
        row = self.db.execute("SELECT URL, metadata, tags, desc FROM bookmarks WHERE id=?", (self.ids[key],)).fetchone()
        return dict(url=row[0], title=row[1], tags=row[2], desc=row[3]) if row else None

    def original(self, key):
        return MEMORIES[key][2]

    def unchanged(self, key):
        m = self.memory(key)
        return bool(m) and m["url"] == self.original(key)

    def in_history(self, key):
        rows = self.db.execute("SELECT URL FROM bookmark_history WHERE bookmark_id=?", (self.ids[key],)).fetchall()
        return self.original(key) in [r[0] for r in rows]

    def verified(self, key):
        row = self.db.execute("SELECT verified_at FROM bookmarks WHERE id=?", (self.ids[key],)).fetchone()
        return bool(row and row[0])

    def reason_recorded(self, key):
        """In the memory, or for a deleted one, in the state that history saved last."""
        m = self.memory(key)
        if m:
            return bool(m["desc"].strip())
        row = self.db.execute(
            "SELECT desc FROM bookmark_history WHERE bookmark_id=? ORDER BY history_id DESC LIMIT 1", (self.ids[key],)
        ).fetchone()
        return bool(row and row[0].strip())

    def mentions(self, text):
        rows = self.db.execute("SELECT URL FROM bookmarks WHERE tags LIKE '%,\\_mem\\_,%' ESCAPE '\\'").fetchall()
        return sum(text in r[0] for r in rows)

    def read(self, name):
        path = self.repo / name
        return path.read_text() if path.exists() else ""

    def changed(self):
        out = sh("git", "status", "--porcelain", cwd=self.repo).stdout
        return [line[3:] for line in out.splitlines()]


def corrected_in_place(s, key, new_text):
    m = s.memory(key)
    return [
        ("corrected_in_the_same_memory", bool(m) and new_text in m["url"] and not s.unchanged(key)),
        ("old_text_kept_in_history", s.in_history(key)),
        ("reason_recorded", s.reason_recorded(key)),
    ]


def true_memories_untouched(s, name):
    """Agents must not correct what the evidence does not reach. VPN is about another system."""
    keys = {"valid_migration", "flaky_tests", "pytest_pref", "noise_vpn", "orjson_pin", "log_path"}
    keys -= {"supersede-on-write": {"orjson_pin"}, "change-trigger": {"log_path"}}.get(name, set())
    changed = sorted(k for k in keys if not s.unchanged(k))
    return ("true_memories_untouched", not changed)


def grade(base: Path, name: str):
    s = State(base)
    if name == "stale-deploy-flag":
        # Only a command counts: an answer may explain in prose why the old flag is gone.
        commands = re.findall(r"deploy\.sh\s+(--[^`\n]*)", s.answer)
        checks = [
            ("acted_on_truth", any("--region eu-west-1" in c and "--legacy-auth" not in c for c in commands)
             and not any("--legacy-auth" in c for c in commands)),
            ("repo_left_unchanged", not s.changed()),
            *corrected_in_place(s, "stale_flag", "--region"),
            ("correction_checked_by_the_agent_is_stamped", s.verified("stale_flag")),
            ("title_no_longer_states_the_old_claim", bool(s.memory("stale_flag")) and s.memory("stale_flag")["title"] != MEMORIES["stale_flag"][1]),
        ]
    elif name == "stale-missing-guard":
        checks = [
            ("acted_on_truth", "DEFAULT_TIMEOUT = 60" in s.read("src/ledgerd/config.py")),
            # Correcting and deleting are both fine: little is left to say about a guard that is gone.
            ("stale_ban_gone", not s.unchanged("stale_guard")),
            ("old_text_kept_in_history", s.in_history("stale_guard")),
            ("reason_recorded", s.reason_recorded("stale_guard")),
        ]
    elif name == "valid-constraint-holds":
        added = [f for f in s.changed() if f.startswith("migrations/") and f != "migrations/0001_init.sql"]
        checks = [
            ("shipped_migration_untouched", "migrations/0001_init.sql" not in s.changed()),
            ("new_migration_adds_the_column", any("currency" in s.read(f) for f in added) or any(
                "currency" in p.read_text() for p in (s.repo / "migrations").glob("*.sql"))),
            ("confirmed_memory_is_stamped", s.verified("valid_migration")),
        ]
    elif name == "supersede-on-write":
        checks = [
            *corrected_in_place(s, "orjson_pin", "3.10.6"),
            ("title_no_longer_states_the_old_claim", bool(s.memory("orjson_pin")) and s.memory("orjson_pin")["title"] != MEMORIES["orjson_pin"][1]),
            ("no_duplicate_claims", s.mentions("3.10.6") == 1),
        ]
    elif name == "change-trigger":
        checks = [
            ("code_changed", "service.log" in s.read("src/ledgerd/logging_setup.py")),
            *corrected_in_place(s, "log_path", "service.log"),
        ]
    elif name == "not-reproduced-keeps-memory":
        checks = [
            # A note may be added, but one passing run does not justify rewriting the claim.
            ("flake_claim_kept", bool(s.memory("flaky_tests")) and s.original("flaky_tests") in s.memory("flaky_tests")["url"]),
            ("answer_reports_the_run", bool(re.search(r"pass|ok|green|healthy", s.answer, re.I))),
        ]
    elif name == "preference-not-overruled":
        checks = [
            ("test_written", any("parse_amount" in s.read(f) for f in s.changed() if f.endswith(".py")) or any(
                "parse_amount" in p.read_text() for p in (s.repo / "tests").glob("*.py"))),
            ("answer_surfaces_the_conflict", bool(re.search(r"pytest", s.answer, re.I) and re.search(r"unittest", s.answer, re.I))),
        ]
    else:
        sys.exit(f"unknown eval: {name}")
    checks.append(true_memories_untouched(s, name))
    # Nothing in the repo can confirm it, so a stamp here means stamping without checking.
    checks.append(("uncheckable_memory_not_stamped", not s.verified("noise_vpn")))
    result = {
        "eval": name,
        "counts_for_stale_rate": name in STALE_EVALS,
        "expectations": [dict(text=t, passed=bool(p), evidence="end state of repo, store and answer") for t, p in checks],
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    command, base = sys.argv[1], Path(sys.argv[2]).resolve()
    if command == "setup":
        setup(base)
    elif command == "prompt":
        print(prompt(base, sys.argv[3]))
    elif command == "grade":
        grade(base, sys.argv[3])
    else:
        sys.exit(__doc__)
