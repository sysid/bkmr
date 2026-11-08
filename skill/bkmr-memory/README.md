# bkmr-memory Skill

Gives AI agents (Claude Code, Copilot CLI, pi-mono) a long-term memory that survives sessions,
stored in [bkmr](https://github.com/sysid/bkmr) instead of `MEMORY.md` files.

## The Problem

Every agent session starts with no memory. The gotcha you hit last week, the deploy procedure you
explained twice, the tool you said you prefer: the next session does not know them. The skill makes
the agent look them up before it works and write them down when it is done.

```
 session 1                         bkmr (_mem_)                  session 2
 ─────────                         ────────────                  ─────────
 hits a bug: CI cache    ──WRITE──> gotcha: CI cache keyed   ──READ──> "fix CI flake"
 keyed on lockfile hash             on lockfile hash                  agent knows the cause
                                                                      before it starts
```

## Using It

Install once by linking the skill where the agent finds it:

```bash
ln -s ~/path/to/bkmr/skill/bkmr-memory ~/.claude/skills/bkmr-memory
```

After that it triggers on its own. You only talk to the agent:

| You say / the agent does | What happens in bkmr |
|---|---|
| Starts a task | `hsearch` for related facts, gotchas, preferences |
| Hits a strange bug, makes a design decision | Searches for prior episodes and decisions |
| "Remember that we deploy with `make release`" | Dedup check, then `add` as `procedure` |
| "What do you know about the auth service?" | `hsearch -t _mem_`, answers from the hits |
| Finishes a task | Stores non-trivial learnings, skips trivia |
| Finds a memory that evidence contradicts | Corrects or deletes it without asking; history keeps the old text |

## The Memory Model

A memory is one bookmark: the content lives in the url field (embedded and printed), tagged with
the `_mem_` system tag and exactly one class.

| Class | For | Review after |
|---|---|---|
| `fact` | Config, architecture, infrastructure truths | 90 days |
| `procedure` | Steps, deployments, workarounds | 180 days |
| `gotcha` | Things that look right but break | 180 days |
| `episode` | Dated debugging stories | 180 days |
| `preference` | Your conventions and tool choices | 365 days |

`project:foo` tags scope a memory to a project. A memory that pins a version or names a harness
(`sandbox`, `mcp`, ...) is reviewed after at most 90 days. `SKILL.md` holds the exact commands,
quoting rules and formatting.

## Keeping It Current

Agents fix memories they use. Memories nobody touches rot silently, so two scripts sweep them:

```
due-report.py ──> which memories are past their review window?     read-only
      │
      v
run-audit.py  ──> headless agent checks the 20 most overdue        writes (undoable),
                  against code, git and config, then per memory:   prints a digest
                    confirmed     -> bkmr verify (stamp)
                    contradicted  -> corrected, evidence in description
                    nothing left  -> deleted
                    unsure        -> kept
                    preference    -> "Needs Tom"
```

```bash
scripts/due-report.py --bkmr ~/bin/bkmr --db ~/path/to/bm.db
scripts/run-audit.py  --bkmr ~/bin/bkmr --db ~/path/to/bm.db --dry-run   # preview, runs nothing
scripts/run-audit.py  --bkmr ~/bin/bkmr --db ~/path/to/bm.db             # digest in ~/.local/state/bkmr-audit/
bkmr restore <history_id>                                                # undo a change from the digest
```

Things to know:

- Pass `--bkmr ~/bin/bkmr`: inside this repo direnv puts a debug build first on PATH. bkmr ≥ 8.2.0
  is required.
- The audit agent runs with `dontAsk`, no user settings and no MCP: read-only tools plus
  `bkmr update/verify/delete --yes`. No file edits, no network, no `bkmr open`.
- It stops after changing more than `min(5, 10%)` of the reviewed memories (exit code 2).
- `claude -p` bills `ANTHROPIC_API_KEY` if set, else your claude.ai login.
- First run: audit a copy (`sqlite3 bm.db ".backup '/tmp/bm-audit.db'"`) and judge the digest.
- Open: no live run has yet confirmed that a user-level `Bash` allow rule stays out; no schedule
  (needs local launchd/cron, a cloud routine cannot reach the DB).

## Developing

- Edit `SKILL.md`; keep it under 500 lines. Agents read top-down, so "Mandatory Tag Rules" stays
  first.
- `make -C skill test` / `make -C skill lint` for the scripts.
- `evals/evals.json` holds agent-behaviour cases; `/skill-creator` runs and grades them. Evals write
  to the DB they are given: use a scratch DB.
- Two evals have their own harness, since `/skill-creator` does not know them:
  - `evals/lifecycle_eval.py setup|prompt|grade <dir> <case>`: stale-memory handling in a
    throwaway repo and store. Keep case names out of the run paths, they give away the answer.
  - `evals/markdown_retrieval.py`: deterministic retrieval of markdown vs. plain content (~4 min).
    Finding: markdown syntax is neutral; multi-fact and truncated documents hurt.

## Why It Is Built This Way

| Choice | Reason |
|---|---|
| Content in the url field, not `-d` | Only url, title and tags are embedded and printed by `_mem_` |
| Exactly one class tag | Enables `-n gotcha` style queries; two classes make a memory ambiguous |
| Dedup before every write | bkmr has no uniqueness constraint; duplicates dilute search |
| `--no-web` on every add | Otherwise bkmr fetches the content as if it were a URL |
| `bkmr update --url`, not `edit` | Non-interactive; agents cannot drive an editor |
| Single quotes around content | Double quotes execute backticks and expand `$` |
