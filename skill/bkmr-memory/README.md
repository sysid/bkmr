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

`project:foo` tags scope a memory to a project. `SKILL.md` holds the exact commands, quoting rules
and formatting.

## Keeping It Current

Agents fix memories they use. Memories nobody touches rot silently, so `due-report.py` lists the
ones past their review window. It only reads the store; reviewing is up to you and the agent of
your choice, under SKILL.md's "Keeping Memory Current" rules.

```
age = today - last `bkmr verify`  (or creation, if never stamped or changed since)

class window ──┬── version in content/title (1.2.3, v2, -rc.4)?  ─┐
 90/180/365    └── harness tag (sandbox, mcp, codex, claude-code)? ┴─> "pinned": window ≤ 90 days

age ≥ window  ──>  due  (means "review", never "delete")
```

Pinned memories describe things that change with releases, not with time, so they come up sooner.
The rule lives in `scripts/due-report.py` (`VERSION`, `HARNESS_TAGS`), computed per run; bkmr
stores nothing about it. Two-part versions like "JDK 25" are not detected.

```bash
scripts/due-report.py --bkmr ~/bin/bkmr --db ~/path/to/bm.db          # table, most overdue first
scripts/due-report.py --bkmr ~/bin/bkmr --db ~/path/to/bm.db --json   # same, for an agent
```

```
    id  over  win from     anchor     chk  title / flags
  3512   121   90 created  2026-04-10 -    Gradle daemon sandbox contamination ...
                                          pinned: version,sandbox
```

`over`: days past the window. `win`: window after pinning. `from`/`anchor`: what age counts from.
`chk`: `yes` if the memory cites a path, commit or URL an agent can recheck.

- Pass `--bkmr ~/bin/bkmr`: inside this repo direnv puts a debug build first on PATH, and
  `BKMR_DB_URL` points at the repo's dev DB. bkmr ≥ 8.2.0 is required.
- `-v` adds debug logging (why each memory is due) to stderr, here and in the eval scripts.

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
